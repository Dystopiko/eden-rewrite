use error_stack::Report;
use error_stack::fmt::{Charset, ColorMode};

/// Configures [`error_stack`] for use in tests by switching to ASCII output
/// and disabling ANSI color codes.
pub fn disable_fancy_error_output() {
    Report::set_charset(Charset::Ascii);
    Report::set_color_mode(ColorMode::None);
}
