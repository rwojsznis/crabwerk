use line_col::LineColLookup;
use ruby_prism::{
    CallNode, ConstantPathNode, ConstantPathTargetNode, KeywordHashNode,
    Location, Node,
};

use crate::{
    Sigil,
    parsing::{ParsedDefinition, Range, UnresolvedReference},
};

use super::inflector::{Acronyms, classify};

#[derive(Debug)]
pub enum ParseError {
    Metaprogramming,
    // Add more variants as needed for different error cases
}

/// prism exposes identifiers and literal contents as raw bytes rather than
/// `String`, because Ruby source is not required to be valid UTF-8.
pub fn bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

pub fn get_definition_from(
    current_nesting: &str,
    parent_nesting: &[String],
    location: &Range,
) -> ParsedDefinition {
    let name = current_nesting.to_owned();

    let owned_namespace_path: Vec<String> = parent_nesting.to_vec();

    let fully_qualified_name = if !owned_namespace_path.is_empty() {
        let mut name_components = owned_namespace_path;
        name_components.push(name);
        format!("::{}", name_components.join("::"))
    } else {
        format!("::{}", name)
    };

    ParsedDefinition {
        fully_qualified_name,
        location: location.to_owned(),
    }
}

pub fn loc_to_range(loc: &Location, lookup: &LineColLookup) -> Range {
    let (start_row, start_col) = lookup.get(loc.start_offset()); // There's an off-by-one difference here with packwerk
    let (end_row, end_col) = lookup.get(loc.end_offset());

    Range {
        start_row,
        start_col: start_col - 1,
        end_row,
        end_col,
    }
}

pub fn fetch_const_name(node: &Node) -> Result<String, ParseError> {
    if let Some(constant_read) = node.as_constant_read_node() {
        return Ok(bytes_to_string(constant_read.name().as_slice()));
    }

    if let Some(constant_path) = node.as_constant_path_node() {
        return fetch_const_path_name(&constant_path);
    }

    Err(ParseError::Metaprogramming)
}

pub fn fetch_const_path_name(
    node: &ConstantPathNode,
) -> Result<String, ParseError> {
    let own_name = node.name().ok_or(ParseError::Metaprogramming)?;

    resolve_const_path(node.parent(), own_name.as_slice())
}

pub fn fetch_const_path_target_name(
    node: &ConstantPathTargetNode,
) -> Result<String, ParseError> {
    let own_name = node.name().ok_or(ParseError::Metaprogramming)?;

    resolve_const_path(node.parent(), own_name.as_slice())
}

/// prism has no dedicated node for a leading `::`. A bare constant is always a
/// `ConstantReadNode`, so an absent parent on a path node unambiguously means
/// the path is rooted at the top level.
fn resolve_const_path(
    parent: Option<Node>,
    own_name: &[u8],
) -> Result<String, ParseError> {
    match parent {
        Some(parent) => {
            let parent_namespace = fetch_const_name(&parent)?;
            Ok(format!(
                "{}::{}",
                parent_namespace,
                bytes_to_string(own_name)
            ))
        }
        None => Ok(format!("::{}", bytes_to_string(own_name))),
    }
}

const ASSOCIATION_METHOD_NAMES: [&str; 4] = [
    "has_one",
    "has_many",
    "belongs_to",
    "has_and_belongs_to_many",
];

pub fn get_reference_from_active_record_association(
    node: &CallNode,
    current_namespaces: &[String],
    line_col_lookup: &LineColLookup,
    custom_associations: &[String],
) -> Option<UnresolvedReference> {
    let method_name = node.name().as_slice();
    let is_association = custom_associations
        .iter()
        .map(String::as_str)
        .chain(ASSOCIATION_METHOD_NAMES.iter().copied())
        .any(|name| name.as_bytes() == method_name);

    if !is_association {
        return None;
    }

    let mut name: Option<String> = None;
    let mut first_arg_symbol: Option<String> = None;

    if let Some(arguments) = node.arguments() {
        for (index, argument) in arguments.arguments().iter().enumerate() {
            if index == 0
                && let Some(symbol) = argument.as_symbol_node()
            {
                first_arg_symbol = Some(bytes_to_string(symbol.unescaped()));
            }

            if let Some(kwargs) = argument.as_keyword_hash_node()
                && let Some(found) = extract_class_name_from_kwargs(&kwargs)
            {
                name = Some(found);
            }
        }
    }

    if name.is_none() {
        // `classify` is what packwerk's AssociationInspector calls, and it
        // singularizes: `has_many :companies` looks for `Company`.
        name = first_arg_symbol.map(|symbol| {
            classify(
                &symbol,
                &Acronyms::default(), // todo: pass in acronyms here
            )
        });
    }

    // Later we should probably handle the cases where we cannot infer a name!
    name.map(|name| UnresolvedReference {
        name,
        namespace_path: current_namespaces.to_owned(),
        location: loc_to_range(&node.location(), line_col_lookup),
    })
}

fn extract_class_name_from_kwargs(kwargs: &KeywordHashNode) -> Option<String> {
    for element in kwargs.elements().iter() {
        let Some(assoc) = element.as_assoc_node() else {
            continue;
        };

        let is_class_name = assoc
            .key()
            .as_symbol_node()
            .is_some_and(|key| key.unescaped() == b"class_name");

        if !is_class_name {
            continue;
        }

        // Handle string literal: class_name: "Foo::Bar"
        if let Some(value) = assoc.value().as_string_node() {
            return Some(bytes_to_string(value.unescaped()));
        }

        // Handle constant with .name: class_name: Foo::Bar.name
        if let Some(call) = assoc.value().as_call_node()
            && call.name().as_slice() == b"name"
            && let Some(receiver) = call.receiver()
            && let Ok(const_name) = fetch_const_name(&receiver)
        {
            return Some(const_name);
        }
    }

    None
}

pub fn extract_sigils_from_contents(contents: &str) -> Vec<Sigil> {
    let mut sigils: Vec<Sigil> = Vec::new();

    // Hardcoded to public, but later we can make this a convention like `pack_*: true`, if we find it more generally useful
    contents.lines().take(5).for_each(|line| {
        if line.contains("pack_public: true") {
            sigils.push(Sigil {
                name: "public".to_string(),
            });
        }
    });

    sigils
}
