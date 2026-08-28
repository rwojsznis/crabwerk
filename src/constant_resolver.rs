use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::parsing::ruby::inflector::Acronyms;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ConstantDefinition {
    pub fully_qualified_name: String,
    pub absolute_path_of_definition: PathBuf,
}

#[derive(Debug)]
pub struct ConstantResolverConfiguration<'a> {
    pub absolute_root: &'a PathBuf,
    pub acronyms: &'a Acronyms,
    pub autoload_roots: &'a HashMap<PathBuf, String>,
}

pub trait ConstantResolver {
    fn resolve(
        &self,
        fully_or_partially_qualified_constant: &str,
        namespace_path: &[&str],
    ) -> Option<Vec<ConstantDefinition>>;

    fn fully_qualified_constant_name_to_constant_definition_map(
        &self,
    ) -> &HashMap<String, Vec<ConstantDefinition>>;
}
