pub mod experimental;
pub mod packwerk;

use std::path::Path;

use crate::file_utils::convert_erb_to_ruby_without_sourcemaps;
use crate::{Configuration, ProcessedFile, parsing::Range};

type RubyParser = fn(String, &Path, &Configuration) -> ProcessedFile;

/// The two ERB parsers differ only in the Ruby parser that reads the converted
/// contents, so both hand that parser to this function. Keeping one body means
/// any later difference between them has to be written on purpose.
fn process_from_contents(
    contents: String,
    path: &Path,
    configuration: &Configuration,
    parse_ruby: RubyParser,
) -> ProcessedFile {
    let ruby_contents = convert_erb_to_ruby_without_sourcemaps(contents);
    let processed_file = parse_ruby(ruby_contents, path, configuration);

    let unresolved_references = processed_file
        .unresolved_references
        .into_iter()
        .map(|mut reference| {
            // Source maps are not yet supported for ERB, since we just turn it
            // into Ruby code that doesn't necessarily map up to the original.
            // We need to add extra logic to support source maps (or use a
            // proper parsing library).
            reference.location = Range::default();
            reference
        })
        .collect();

    ProcessedFile {
        absolute_path: path.to_path_buf(),
        unresolved_references,
        // A template defines no constant: Zeitwerk never autoloads an `.erb`
        // file, so whatever the Ruby parser read out of the converted contents
        // cannot be a definition.
        definitions: vec![],
        sigils: processed_file.sigils,
    }
}
