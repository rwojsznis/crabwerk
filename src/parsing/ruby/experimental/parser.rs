use crate::file_utils::file_read_contents;
use crate::parsing::ruby::parse_utils::extract_sigils_from_contents;
use crate::{
    Configuration, ProcessedFile,
    parsing::{
        ParsedDefinition, UnresolvedReference,
        ruby::parse_utils::{
            bytes_to_string, fetch_const_name, fetch_const_path_name,
            fetch_const_path_target_name, get_definition_from,
            get_reference_from_active_record_association, loc_to_range,
        },
    },
};
use line_col::LineColLookup;
use ruby_prism::{
    CallNode, ClassNode, ConstantAndWriteNode, ConstantOperatorWriteNode,
    ConstantOrWriteNode, ConstantPathAndWriteNode, ConstantPathNode,
    ConstantPathOperatorWriteNode, ConstantPathOrWriteNode,
    ConstantPathTargetNode, ConstantPathWriteNode, ConstantReadNode,
    ConstantTargetNode, ConstantWriteNode, DefNode, Location, ModuleNode,
    Visit, parse,
};
use std::path::Path;

struct ReferenceCollector<'a> {
    pub references: Vec<UnresolvedReference>,
    pub definitions: Vec<ParsedDefinition>,
    pub current_namespaces: Vec<String>,
    pub line_col_lookup: LineColLookup<'a>,
    pub behavioral_change_in_namespace: bool,
    pub custom_associations: Vec<String>,
    pub is_spec_file: bool,
}

impl ReferenceCollector<'_> {
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

        let namespace_path = self
            .current_namespaces
            .clone()
            .into_iter()
            .filter(|namespace| namespace != &name)
            .collect::<Vec<String>>();

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
            self.visit(&superclass);
        }

        let location =
            loc_to_range(&constant_path.location(), &self.line_col_lookup);

        let definition = get_definition_from(
            &namespace,
            &self.current_namespaces,
            &location,
        );

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        // Each time we open up a new class/module, we reset the behavioral change flag
        let previous_behavioral_change = self.behavioral_change_in_namespace;
        self.behavioral_change_in_namespace = false;

        if let Some(body) = node.body() {
            self.visit(&body);
        }

        if self.behavioral_change_in_namespace {
            self.definitions.push(definition);
        }

        // When we're done visiting the class/module, we restore the previous behavioral change flag
        // to account for nested class/module definitions
        self.behavioral_change_in_namespace = previous_behavioral_change;

        self.current_namespaces.pop();
    }

    fn visit_module_node(&mut self, node: &ModuleNode<'pr>) {
        let constant_path = node.constant_path();
        let namespace = fetch_const_name(&constant_path).unwrap_or_default();

        let location =
            loc_to_range(&constant_path.location(), &self.line_col_lookup);

        let definition = get_definition_from(
            &namespace,
            &self.current_namespaces,
            &location,
        );

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        // Each time we open up a new class/module, we reset the behavioral change flag
        let previous_behavioral_change = self.behavioral_change_in_namespace;
        self.behavioral_change_in_namespace = false;

        if let Some(body) = node.body() {
            self.visit(&body);
        }

        if self.behavioral_change_in_namespace {
            self.definitions.push(definition);
        }

        // When we're done visiting the class/module, we restore the previous behavioral change flag
        // to account for nested class/module definitions
        self.behavioral_change_in_namespace = previous_behavioral_change;

        self.current_namespaces.pop();
    }

    fn visit_call_node(&mut self, node: &CallNode<'pr>) {
        // `private_constant`, RSpec methods, and anything inside RSpec describe blocks
        // are not considered to be behavioral changes
        if node.name().as_slice() != b"private_constant" && !self.is_spec_file {
            self.behavioral_change_in_namespace = true;

            let association_reference =
                get_reference_from_active_record_association(
                    node,
                    &self.current_namespaces,
                    &self.line_col_lookup,
                    &self.custom_associations,
                );

            if let Some(association_reference) = association_reference {
                self.references.push(association_reference);
            }
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

    // prism represents instance and singleton method definitions with one node,
    // where lib-ruby-parser had `Def` and `Defs`.
    fn visit_def_node(&mut self, node: &DefNode<'pr>) {
        if !self.is_spec_file {
            self.behavioral_change_in_namespace = true;
        }

        ruby_prism::visit_def_node(self, node);
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

    /*
       `crabwerk` has a feature that detects a monkey patch within a module, e.g.:
       module SomeOtherPack
         some_monkey_patch
       end

       It then considers this a definition of `SomeOtherPack`. This is a bit idiosyncratic – but was intended to support experimental detection
       of monkey patches, e.g. to String.

       This causes issues for a common RSpec pattern:

       module MyModule
         RSpec.describe MyClass do
           ...
         end
       end

       To address this, we disable the monkey patch detection in spec files.
    */
    let is_spec_file = path.to_string_lossy().contains("_spec.rb")
        || path.to_string_lossy().contains("/spec/");

    let mut collector = ReferenceCollector {
        references: vec![],
        current_namespaces: vec![],
        definitions: vec![],
        line_col_lookup: LineColLookup::new(&contents),
        behavioral_change_in_namespace: false,
        custom_associations: configuration.custom_associations.clone(),
        is_spec_file,
    };

    collector.visit(&parse_result.node());

    let unresolved_references = collector.references;

    let absolute_path = path.to_owned();

    // The packwerk parser uses a ConstantResolver constructed by constants inferred from the file system
    // see zeitwerk_utils for more.
    // For a parser that uses parsed constants, see the experimental parser
    let definitions = collector.definitions;

    let sigils = extract_sigils_from_contents(&contents);

    ProcessedFile {
        absolute_path,
        unresolved_references,
        definitions,
        sigils,
    }
}
