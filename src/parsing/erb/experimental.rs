pub mod parser;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::parsing::erb::experimental::parser::process_from_contents;
    use crate::{Configuration, Sigil};

    #[test]
    fn sigil_in_an_erb_comment() {
        let contents: String =
            String::from("<% # pack_public: true %>\n<%= Foo %>");
        let configuration = Configuration::default();

        assert_eq!(
            vec![Sigil {
                name: String::from("public")
            }],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.erb"),
                &configuration
            )
            .sigils
        );
    }
}
