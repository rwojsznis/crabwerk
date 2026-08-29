use tracing::Level;
use tracing::metadata::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;

/// Hide dependency logs that obscure crabwerk's debug spans.
fn log_filter() -> tracing_subscriber::filter::Targets {
    tracing_subscriber::filter::Targets::new()
        .with_default(LevelFilter::DEBUG)
        .with_target("globset", LevelFilter::OFF)
        .with_target("ignore", LevelFilter::OFF)
}

pub fn install_logger(debug: bool) {
    let filter = log_filter();

    let subscriber_builder = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_level(true)
        .with_writer(std::io::stderr)
        .with_file(true)
        .with_span_events(FmtSpan::ACTIVE)
        .with_line_number(true);

    if debug {
        // `install_logger` is the first thing `cli::run` does, before any
        // worker threads exist, so no other thread can be reading the
        // environment while this writes to it.
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };

        let subscriber_builder =
            subscriber_builder.with_max_level(Level::DEBUG);
        let subscriber = subscriber_builder.finish();
        let layered_subscriber = filter.with_subscriber(subscriber);
        layered_subscriber.init();
    } else {
        let subscriber = subscriber_builder.finish();
        let layered_subscriber = filter.with_subscriber(subscriber);
        layered_subscriber.init();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_filter_silences_noisy_dependencies() {
        let filter = log_filter();

        assert!(filter.would_enable("crabwerk::walk_directory", &Level::DEBUG));
        assert!(!filter.would_enable("globset", &Level::DEBUG));
        assert!(!filter.would_enable("ignore::walk", &Level::DEBUG));
    }
}
