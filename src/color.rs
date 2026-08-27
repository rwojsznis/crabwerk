use std::io::IsTerminal;

/// When to write ANSI colour codes to stdout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    /// A flag the user typed beats a variable the shell exported, so
    /// `NO_COLOR` applies to `auto` only.
    const fn resolve(self, is_terminal: bool, no_color: bool) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => is_terminal && !no_color,
        }
    }

    /// The answer for this process.
    pub(crate) fn enabled(self) -> bool {
        // https://no-color.org: any value other than the empty string counts.
        let no_color =
            std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());

        self.resolve(std::io::stdout().is_terminal(), no_color)
    }
}

#[cfg(test)]
mod tests {
    use super::ColorChoice;

    #[test]
    fn auto_colours_a_terminal_only() {
        assert!(ColorChoice::Auto.resolve(true, false));
        assert!(!ColorChoice::Auto.resolve(false, false));
    }

    #[test]
    fn no_color_suppresses_auto() {
        assert!(!ColorChoice::Auto.resolve(true, true));
    }

    #[test]
    fn explicit_choices_ignore_the_environment() {
        assert!(ColorChoice::Always.resolve(false, true));
        assert!(!ColorChoice::Never.resolve(true, false));
    }
}
