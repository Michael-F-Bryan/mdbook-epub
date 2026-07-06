use std::sync::Once;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

const MODULE_LOG_FILTERS: &str = concat!(
    "ERROR,",
    "mdbook-epub=ERROR,",
    "epub_builder=ERROR,",
    "handlebars=ERROR,",
    "mdbook_core=ERROR,",
    "mdbook_renderer=ERROR,",
    "pulldown_cmark=ERROR,",
    "ureq=ERROR,",
    "ureq_proto=ERROR",
);

static INIT: Once = Once::new();

pub(crate) fn init_tracing() {
    INIT.call_once(|| {
        let fmt_layer = fmt::layer()
            .with_level(true) // Show the logging level
            .with_ansi(true) // Turn on color (for readability)
            .event_format(tracing_subscriber::fmt::format().compact()) // Compact log format
            .compact();

        let env_filter = match std::env::var("RUST_LOG") {
            // You can pass your own set of filters via an environment variable
            Ok(_) => EnvFilter::from_env("RUST_LOG"),
            // If RUST_LOG is missing, MODULE_LOG_FILTERS will be loaded by default.
            Err(_) => EnvFilter::new(MODULE_LOG_FILTERS),
        };

        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(env_filter)
            .init();
    });
}
