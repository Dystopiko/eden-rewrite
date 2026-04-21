use error_stack::Report;
use error_stack::fmt::{Charset, ColorMode};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

/// Configures [`error_stack`] for use in tests by switching to ASCII output
/// and disabling ANSI color codes.
pub fn disable_fancy_error_output() {
    Report::set_charset(Charset::Ascii);
    Report::set_color_mode(ColorMode::None);
}

/// Initializes [`tracing_subscriber`] for tests.
pub fn init_tracing_for_tests() {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env_lossy();

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_test_writer()
        .try_init();
}
