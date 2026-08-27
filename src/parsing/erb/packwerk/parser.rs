use crate::file_utils::file_read_contents;
use crate::parsing::ruby::packwerk::parser::process_from_contents as process_from_ruby_contents;
use crate::{Configuration, ProcessedFile};
use std::path::Path;

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
    super::super::process_from_contents(
        contents,
        path,
        configuration,
        process_from_ruby_contents,
    )
}
