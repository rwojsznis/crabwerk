use crate::file_utils::file_read_contents;
use crate::parsing::ruby::inflector::Acronyms;
use crate::parsing::ruby::parse_utils::extract_sigils_from_contents;

use crate::{
    Configuration, ProcessedFile,
    parsing::{
        ParsedDefinition, Range, UnresolvedReference,
        ruby::{
            namespace_calculator::possible_fully_qualified_constants,
            parse_utils::{
                bytes_to_string, fetch_const_name, fetch_const_path_name,
                fetch_const_path_target_name, get_definition_from,
                get_reference_from_active_record_association, loc_to_range,
            },
        },
    },
};
use line_col::LineColLookup;
use ruby_prism::{
    CallNode, ClassNode, ConstantAndWriteNode, ConstantOperatorWriteNode,
    ConstantOrWriteNode, ConstantPathAndWriteNode, ConstantPathNode,
    ConstantPathOperatorWriteNode, ConstantPathOrWriteNode,
    ConstantPathTargetNode, ConstantPathWriteNode, ConstantReadNode,
    ConstantTargetNode, ConstantWriteNode, Location, ModuleNode, Visit, parse,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SuperclassReference {
    pub name: String,
    pub namespace_path: Vec<String>,
}

struct ReferenceCollector<'a> {
    pub references: Vec<UnresolvedReference>,
    pub definitions: Vec<ParsedDefinition>,
    pub current_namespaces: Vec<String>,
    pub line_col_lookup: LineColLookup<'a>,
    pub in_superclass: bool,
    pub superclasses: Vec<SuperclassReference>,
    pub custom_associations: Vec<String>,
    pub acronyms: &'a Acronyms,
}

impl ReferenceCollector<'_> {
    fn push_namespace_definition(&mut self, namespace: &str, location: &Range) {
        let definition =
            get_definition_from(namespace, &self.current_namespaces, location);

        let name = definition.fully_qualified_name.to_owned();
        let namespace_path = self.current_namespaces.to_owned();
        self.definitions.push(definition);

        // Packwerk also considers a definition to be a "reference"
        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location: location.to_owned(),
        });
    }

    /// The twelve constant assignment node types share no accessor trait, so
    /// each visitor resolves its own name and funnels through here.
    fn push_constant_assignment(&mut self, name: &str, location: &Location) {
        let location = loc_to_range(location, &self.line_col_lookup);

        self.definitions.push(get_definition_from(
            name,
            &self.current_namespaces,
            &location,
        ));
    }

    fn push_constant_reference(&mut self, name: String, location: &Location) {
        let location = loc_to_range(location, &self.line_col_lookup);

        if self.in_superclass {
            self.superclasses.push(SuperclassReference {
                name: name.to_owned(),
                namespace_path: self.current_namespaces.to_owned(),
            })
        }
        // In packwerk, NodeHelpers.enclosing_namespace_path ignores
        // namespaces where a superclass OR namespace is the same as the current reference name
        let matching_superclass_option = self
            .superclasses
            .iter()
            .find(|superclass| superclass.name == name);

        let namespace_path =
            if let Some(matching_superclass) = matching_superclass_option {
                matching_superclass.namespace_path.to_owned()
            } else {
                self.current_namespaces
                    .clone()
                    .into_iter()
                    .filter(|namespace| {
                        namespace != &name
                            || self
                                .superclasses
                                .iter()
                                .any(|superclass| superclass.name == name)
                    })
                    .collect::<Vec<String>>()
            };

        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location,
        })
    }
}

impl<'pr> Visit<'pr> for ReferenceCollector<'_> {
    fn visit_class_node(&mut self, node: &ClassNode<'pr>) {
        let constant_path = node.constant_path();

        // For now, we simply exit and stop traversing if we encounter an error when fetching the constant name of a class
        // We can iterate on this if this is different than the packwerk implementation
        let Ok(namespace) = fetch_const_name(&constant_path) else {
            return;
        };

        if let Some(superclass) = node.superclass() {
            self.in_superclass = true;
            self.visit(&superclass);
            self.in_superclass = false;
        }

        let location =
            loc_to_range(&constant_path.location(), &self.line_col_lookup);

        self.push_namespace_definition(&namespace, &location);

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        if let Some(body) = node.body() {
            self.visit(&body);
        }

        self.current_namespaces.pop();
        self.superclasses.pop();
    }

    fn visit_module_node(&mut self, node: &ModuleNode<'pr>) {
        let constant_path = node.constant_path();

        // A module name is unresolvable only in a partially recovered tree,
        // where prism substitutes a placeholder node for the missing name.
        let Ok(namespace) = fetch_const_name(&constant_path) else {
            return;
        };

        let location =
            loc_to_range(&constant_path.location(), &self.line_col_lookup);

        self.push_namespace_definition(&namespace, &location);

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        if let Some(body) = node.body() {
            self.visit(&body);
        }

        self.current_namespaces.pop();
    }

    fn visit_call_node(&mut self, node: &CallNode<'pr>) {
        let association_reference =
            get_reference_from_active_record_association(
                node,
                &self.current_namespaces,
                &self.line_col_lookup,
                &self.custom_associations,
                self.acronyms,
            );

        if let Some(association_reference) = association_reference {
            self.references.push(association_reference);
        }

        ruby_prism::visit_call_node(self, node);
    }

    fn visit_constant_write_node(&mut self, node: &ConstantWriteNode<'pr>) {
        let name = bytes_to_string(node.name().as_slice());
        self.push_constant_assignment(&name, &node.location());

        self.visit(&node.value());
    }

    fn visit_constant_or_write_node(
        &mut self,
        node: &ConstantOrWriteNode<'pr>,
    ) {
        let name = bytes_to_string(node.name().as_slice());
        self.push_constant_assignment(&name, &node.location());

        self.visit(&node.value());
    }

    fn visit_constant_and_write_node(
        &mut self,
        node: &ConstantAndWriteNode<'pr>,
    ) {
        let name = bytes_to_string(node.name().as_slice());
        self.push_constant_assignment(&name, &node.location());

        self.visit(&node.value());
    }

    fn visit_constant_operator_write_node(
        &mut self,
        node: &ConstantOperatorWriteNode<'pr>,
    ) {
        let name = bytes_to_string(node.name().as_slice());
        self.push_constant_assignment(&name, &node.location());

        self.visit(&node.value());
    }

    // Assignment targets, e.g. `A, B = 1, 2` or `for X in [1]`, carry no value
    // to traverse into.
    fn visit_constant_target_node(&mut self, node: &ConstantTargetNode<'pr>) {
        let name = bytes_to_string(node.name().as_slice());
        self.push_constant_assignment(&name, &node.location());
    }

    fn visit_constant_path_target_node(
        &mut self,
        node: &ConstantPathTargetNode<'pr>,
    ) {
        if let Ok(name) = fetch_const_path_target_name(node) {
            self.push_constant_assignment(&name, &node.location());
        }
    }

    // The scoped assignment family below deliberately does not visit `target`.
    // Doing so would emit a constant *reference* for the assignee, which the
    // single `Casgn` node this replaces never produced.
    fn visit_constant_path_write_node(
        &mut self,
        node: &ConstantPathWriteNode<'pr>,
    ) {
        if let Ok(name) = fetch_const_path_name(&node.target()) {
            self.push_constant_assignment(&name, &node.location());
        }

        self.visit(&node.value());
    }

    fn visit_constant_path_or_write_node(
        &mut self,
        node: &ConstantPathOrWriteNode<'pr>,
    ) {
        if let Ok(name) = fetch_const_path_name(&node.target()) {
            self.push_constant_assignment(&name, &node.location());
        }

        self.visit(&node.value());
    }

    fn visit_constant_path_and_write_node(
        &mut self,
        node: &ConstantPathAndWriteNode<'pr>,
    ) {
        if let Ok(name) = fetch_const_path_name(&node.target()) {
            self.push_constant_assignment(&name, &node.location());
        }

        self.visit(&node.value());
    }

    fn visit_constant_path_operator_write_node(
        &mut self,
        node: &ConstantPathOperatorWriteNode<'pr>,
    ) {
        if let Ok(name) = fetch_const_path_name(&node.target()) {
            self.push_constant_assignment(&name, &node.location());
        }

        self.visit(&node.value());
    }

    fn visit_constant_read_node(&mut self, node: &ConstantReadNode<'pr>) {
        let name = bytes_to_string(node.name().as_slice());
        self.push_constant_reference(name, &node.location());
    }

    fn visit_constant_path_node(&mut self, node: &ConstantPathNode<'pr>) {
        match fetch_const_path_name(node) {
            // Not recursing is what keeps `Foo::Bar` from also emitting `Foo`.
            Ok(name) => self.push_constant_reference(name, &node.location()),
            // A dynamic path such as `self.class::CONST` is not itself a
            // reference, but its parent may still contain one.
            Err(_) => {
                if let Some(parent) = node.parent() {
                    self.visit(&parent);
                }
            }
        }
    }
}

pub fn process_from_path(
    path: &Path,
    configuration: &Configuration,
) -> anyhow::Result<ProcessedFile> {
    let contents = file_read_contents(path)?;
    Ok(process_from_contents(contents, path, configuration))
}

pub fn process_from_contents(
    contents: String,
    path: &Path,
    configuration: &Configuration,
) -> ProcessedFile {
    // prism recovers from syntax errors and returns a partial tree, which we
    // traverse as-is rather than discarding. Recovery is what lets ERB work at
    // all, since converting a template to Ruby drops the tags that balanced it.
    let parse_result = parse(contents.as_bytes());

    let mut collector = ReferenceCollector {
        references: vec![],
        current_namespaces: vec![],
        definitions: vec![],
        line_col_lookup: LineColLookup::new(&contents),
        in_superclass: false,
        superclasses: vec![],
        custom_associations: configuration.custom_associations.clone(),
        acronyms: &configuration.acronyms,
    };

    collector.visit(&parse_result.node());

    let mut definition_to_location_map: HashMap<String, Range> = HashMap::new();

    for d in &collector.definitions {
        let parts: Vec<&str> = d.fully_qualified_name.split("::").collect();
        // We do this to handle nested constants, e.g.
        // class Foo::Bar
        // end
        for (index, _) in parts.iter().enumerate() {
            let combined = &parts[..=index].join("::");
            // If the map already contains the key, skip it.
            // This is helpful, e.g.
            // class Foo::Bar
            //  BAZ
            // end
            // The fully name for BAZ IS ::Foo::Bar::BAZ, so we do not want to overwrite
            // the definition location for ::Foo or ::Foo::Bar
            if !definition_to_location_map.contains_key(combined) {
                definition_to_location_map
                    .insert(combined.to_owned(), d.location.clone());
            }
        }
    }

    let unresolved_references = collector
        .references
        .into_iter()
        .filter(|r| {
            let namespace_path = r
                .namespace_path
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<&str>>();
            let possible_constants =
                possible_fully_qualified_constants(&namespace_path, &r.name);
            // `local_reference?` in lib/packwerk/parsed_constant_definitions.rb
            // is an `any?` over the candidate names, and it does not count a
            // reference that sits where the definition does. We match that
            // shape so a single candidate cannot undo an earlier match.
            let should_ignore_local_reference =
                possible_constants.iter().any(|constant_name| {
                    definition_to_location_map.get(constant_name).is_some_and(
                        |location| {
                            location.start_row != r.location.start_row
                                || location.start_col != r.location.start_col
                        },
                    )
                });
            !should_ignore_local_reference
        })
        .collect();

    let absolute_path = path.to_owned();

    // The packwerk parser uses a ConstantResolver constructed by constants inferred from the file system
    // see zeitwerk_utils for more.
    // For a parser that uses parsed constants, see the experimental parser
    let definitions = vec![];

    let sigils = extract_sigils_from_contents(&contents);

    ProcessedFile {
        absolute_path,
        unresolved_references,
        definitions,
        sigils,
    }
}
