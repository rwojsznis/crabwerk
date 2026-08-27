use anyhow::bail;
use tracing::debug;

use std::collections::HashMap;
use std::path::Path;

use crate::{
    constant_resolver::{ConstantDefinition, ConstantResolver},
    parsing::ruby::namespace_calculator::combine_namespace_with_constant_name,
};

#[derive(Default, Debug)]
pub struct ZeitwerkConstantResolver {
    pub fully_qualified_constant_name_to_constant_definition_map:
        HashMap<String, Vec<ConstantDefinition>>,
}

impl ConstantResolver for ZeitwerkConstantResolver {
    fn resolve(
        &self,
        fully_or_partially_qualified_constant: &str,
        namespace_path: &[&str],
    ) -> Option<Vec<ConstantDefinition>> {
        // If the fully_or_partially_qualified_constant is prefixed with ::, the namespace path is technically empty, since it's a global reference
        let (namespace_path, const_name) =
            if fully_or_partially_qualified_constant.starts_with("::") {
                // `resolve_constant` will add a leading :: before it makes a guess at the fully qualified name
                // so we remove it here and represent it as a relative constant with no namespace path
                let const_name = fully_or_partially_qualified_constant
                    .strip_prefix("::")
                    .unwrap();
                let namespace_path: &[&str] = &[];
                (namespace_path, const_name)
            } else {
                (namespace_path, fully_or_partially_qualified_constant)
            };

        self.resolve_constant(const_name, namespace_path, const_name)
    }

    fn fully_qualified_constant_name_to_constant_definition_map(
        &self,
    ) -> &HashMap<String, Vec<ConstantDefinition>> {
        &self.fully_qualified_constant_name_to_constant_definition_map
    }
}

impl ZeitwerkConstantResolver {
    pub fn create(
        constants: Vec<ConstantDefinition>,
        absolute_root: &Path,
    ) -> anyhow::Result<Box<dyn ConstantResolver + Send + Sync>> {
        debug!("Building constant resolver from constants vector");

        let mut fully_qualified_constant_to_constant_map: HashMap<
            String,
            Vec<ConstantDefinition>,
        > = HashMap::new();

        for constant in constants {
            fully_qualified_constant_to_constant_map
                .entry(constant.fully_qualified_name.clone())
                .or_default()
                .push(constant);
        }

        // packwerk's resolver collects every ambiguity before it raises, so
        // one run tells the user about all of them.
        let mut ambiguous: Vec<(&str, Vec<String>)> =
            fully_qualified_constant_to_constant_map
                .iter()
                .filter(|(_name, definitions)| definitions.len() > 1)
                .map(|(name, definitions)| {
                    let mut paths: Vec<String> = definitions
                        .iter()
                        .map(|definition| {
                            let path = &definition.absolute_path_of_definition;
                            path.strip_prefix(absolute_root)
                                .unwrap_or(path)
                                .to_string_lossy()
                                .to_string()
                        })
                        .collect();
                    // The definitions arrive from a parallel walk, so neither
                    // the paths nor the constants they belong to have an order
                    // of their own.
                    paths.sort();
                    (name.as_str(), paths)
                })
                .collect();

        if !ambiguous.is_empty() {
            ambiguous.sort_by_key(|(name, _)| *name);

            let details = ambiguous
                .iter()
                .map(|(name, paths)| {
                    // The gem names the constant without its leading `::`.
                    format!(
                        "\"{}\" could refer to any of\n  {}",
                        name.trim_start_matches("::"),
                        paths.join("\n  ")
                    )
                })
                .collect::<Vec<String>>()
                .join("\n");

            bail!("Ambiguous constant definition:\n\n{}", details);
        }

        debug!("Finished building constant resolver");

        Ok(Box::new(Self {
            fully_qualified_constant_name_to_constant_definition_map:
                fully_qualified_constant_to_constant_map,
        }))
    }

    fn resolve_constant<'a>(
        &'a self,
        const_name: &'a str,
        current_namespace_path: &'a [&str],
        original_name: &'a str,
    ) -> Option<Vec<ConstantDefinition>> {
        let constant = self.resolve_traversing_namespace_path(
            const_name,
            current_namespace_path,
            original_name,
        );
        match constant {
            Some(definition) => Some(vec![definition]),
            None => {
                // If we couldn't find a match, it's possible the constant is defined within its parent namespace and not within its own file.
                // For example, `Boo` above could be defined in `foo/bar.rb` as:
                // module Foo
                //   module Bar
                //     class Boo
                //     end
                //   end
                // end
                // Therefore, we take the given const_name, remove the last part of the fully qualified name, and try again.
                // In this case, we'd try to resolve `::Foo::Bar` instead of `::Foo::Bar::Boo`
                let split_const = const_name.split("::").collect::<Vec<&str>>();
                if split_const.len() <= 1 {
                    return None;
                }
                let parent_constant =
                    split_const[0..=split_const.len() - 2].join("::");
                self.resolve_constant(
                    &parent_constant,
                    current_namespace_path,
                    original_name,
                )
            }
        }
    }

    // In Ruby, say we have this code:
    //
    // module Foo
    //   module Bar
    //     module Baz
    //       Boo
    //     end
    //   end
    // end
    //
    // The `current_namespace_path` here is: ['Foo', 'Bar', 'Baz']
    // The `const_name` here is: `Boo`
    // Ruby constant resolution rules dictate that `Boo` coudl refer to any of the following,
    // in this specific order:
    //
    // ::Foo::Bar::Baz::Boo
    // ::Foo::Bar::Boo
    // ::Foo::Boo
    // ::Boo
    //
    // We need to check each of these possibilities in order, and return the first one that exists
    // If none of them exist, return None
    fn resolve_traversing_namespace_path<'a>(
        &'a self,
        const_name: &'a str,
        current_namespace_path: &'a [&str],
        original_name: &'a str,
    ) -> Option<ConstantDefinition> {
        let fully_qualified_name_guess = combine_namespace_with_constant_name(
            current_namespace_path,
            const_name,
        );

        let Some(constant) =
            self.constant_for_fully_qualified_name(&fully_qualified_name_guess)
        else {
            // In this case, we couldn't find a constant with the given name under the given namespace.
            // However, it's possible the constant is defined within the parent namespace.
            return current_namespace_path.split_last().and_then(
                |(_last, parent_namespace)| {
                    self.resolve_traversing_namespace_path(
                        const_name,
                        parent_namespace,
                        original_name,
                    )
                },
            );
        };

        // Since the ContantResolver might say that some constant Foo::Bar::Baz is defined in Foo::Bar,
        // we want to return a ConstantDefinition that has the fully qualified name of the constant we're looking for.
        // In this case, we want to return a ConstantDefinition with the fully qualified name of Foo::Bar::Baz
        // even though the ConstantDefinition we found has the fully qualified name of Foo::Bar
        // The ConstantResolver from the experimental parser does not need to do this, so we might be better off
        // having a separate ConstantResolver for that implementation
        let fully_qualified_name = combine_namespace_with_constant_name(
            current_namespace_path,
            original_name,
        );

        let absolute_path_of_definition =
            constant.absolute_path_of_definition.to_owned();
        Some(ConstantDefinition {
            fully_qualified_name,
            absolute_path_of_definition,
        })
    }

    fn constant_for_fully_qualified_name(
        &self,
        fully_qualified_name: &String,
    ) -> Option<&ConstantDefinition> {
        self.fully_qualified_constant_name_to_constant_definition_map
            .get(fully_qualified_name)
            .and_then(|definitions| definitions.first())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn definition(
        fully_qualified_name: &str,
        relative_path: &str,
    ) -> ConstantDefinition {
        ConstantDefinition {
            fully_qualified_name: fully_qualified_name.to_owned(),
            absolute_path_of_definition: PathBuf::from("/app")
                .join(relative_path),
        }
    }

    #[test]
    fn create_resolves_a_single_definition() {
        let resolver = ZeitwerkConstantResolver::create(
            vec![definition("::Foo", "packs/a/app/services/foo.rb")],
            Path::new("/app"),
        )
        .unwrap();

        assert_eq!(
            resolver.resolve("Foo", &[]),
            Some(vec![definition("::Foo", "packs/a/app/services/foo.rb")])
        );
    }

    #[test]
    fn create_reports_every_ambiguous_constant() {
        let result = ZeitwerkConstantResolver::create(
            vec![
                definition("::Foo", "packs/b/app/services/foo.rb"),
                definition("::Foo", "packs/a/app/services/foo.rb"),
                definition("::Bar", "packs/b/app/services/bar.rb"),
                definition("::Bar", "packs/a/app/services/bar.rb"),
                definition("::Baz", "packs/a/app/services/baz.rb"),
            ],
            Path::new("/app"),
        );

        let Err(error) = result else {
            panic!("expected an ambiguous constant error");
        };

        assert_eq!(
            error.to_string(),
            "Ambiguous constant definition:\n\
             \n\
             \"Bar\" could refer to any of\n  \
             packs/a/app/services/bar.rb\n  \
             packs/b/app/services/bar.rb\n\
             \"Foo\" could refer to any of\n  \
             packs/a/app/services/foo.rb\n  \
             packs/b/app/services/foo.rb"
        );
    }
}
