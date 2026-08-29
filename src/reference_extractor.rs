use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use crate::{
    ProcessedFile, get_experimental_constant_resolver,
    get_zeitwerk_constant_resolver, process_files,
};

use super::{Configuration, Sigil, checker::reference::Reference};

#[allow(clippy::type_complexity)]
pub fn get_all_references_and_sigils(
    configuration: &Configuration,
    absolute_paths: &HashSet<PathBuf>,
) -> anyhow::Result<(Vec<Reference>, HashMap<PathBuf, Vec<Sigil>>)> {
    let (constant_resolver, processed_files_to_check) =
        if configuration.experimental_parser {
            // The experimental resolver gets definitions from every parsed file.
            let all_processed_files: Vec<ProcessedFile> =
                process_files(&configuration.included_files, configuration)?;

            let constant_resolver = get_experimental_constant_resolver(
                &configuration.absolute_root,
                &all_processed_files,
                &configuration.ignored_definitions,
            );

            let processed_files_to_check = all_processed_files
                .into_iter()
                .filter(|processed_file| {
                    absolute_paths.contains(&processed_file.absolute_path)
                })
                .collect();

            (constant_resolver, processed_files_to_check)
        } else {
            let processed_files: Vec<ProcessedFile> =
                process_files(absolute_paths, configuration)?;

            let constant_resolver = get_zeitwerk_constant_resolver(
                &configuration.pack_set,
                &configuration.constant_resolver_configuration(),
            )?;

            (constant_resolver, processed_files)
        };

    let mut path_to_sigils: HashMap<PathBuf, Vec<Sigil>> = HashMap::new();
    for processed_file in &processed_files_to_check {
        if !processed_file.sigils.is_empty() {
            path_to_sigils.insert(
                processed_file.absolute_path.to_owned(),
                processed_file.sigils.to_owned(),
            );
        }
    }

    let references: anyhow::Result<Vec<Reference>> = processed_files_to_check
        .par_iter()
        .try_fold(Vec::new, |mut acc, processed_file| {
            for unresolved_ref in &processed_file.unresolved_references {
                let mut refs = Reference::from_unresolved_reference(
                    configuration,
                    constant_resolver.as_ref(),
                    unresolved_ref,
                    &processed_file.absolute_path,
                )?;
                acc.append(&mut refs);
            }
            Ok(acc)
        })
        .try_reduce(Vec::new, |mut acc, mut vec| {
            acc.append(&mut vec);
            Ok(acc)
        });

    Ok((references?, path_to_sigils))
}
