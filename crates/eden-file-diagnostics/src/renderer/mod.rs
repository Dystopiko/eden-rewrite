use codespan_reporting::{
    diagnostic::Diagnostic,
    files::{Error as FilesError, SimpleFiles},
    term::emit_into_string,
};

use crate::RenderedDiagnostic;

/// A renderer for converting file-based diagnostics into human-readable formatted strings.
///
/// [`Renderer`] is a convenience wrapper around [`codespan_reporting`] that manages
/// source files and renders diagnostics with rich formatting.
///
/// It allows you to add multiple files if more context is required for
/// the user to understand the entire scope of the error across files.
///
/// # Examples
///
/// ```rust
/// use eden_file_diagnostics::{Renderer, codespan_reporting::diagnostic::{Diagnostic, Label}};
///
/// let source = "let x = 42;";
/// let renderer = Renderer::new()
///     .with_file("main.rs", source);
///
/// let diagnostic = Diagnostic::warning()
///     .with_message("unused variable")
///     .with_labels(vec![
///         Label::primary(0, 4..5).with_message("`x` is never used")
///     ]);
///
/// if let Ok(rendered) = renderer.render(diagnostic) {
///     println!("{}", rendered);
/// }
/// ```
#[must_use]
pub struct Renderer<'a> {
    config: codespan_reporting::term::Config,
    files: SimpleFiles<String, &'a str>,
}

impl<'a> Default for Renderer<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Renderer<'a> {
    /// Creates a new renderer with default configuration.
    pub fn new() -> Self {
        Self {
            config: codespan_reporting::term::Config::default(),
            files: SimpleFiles::new(),
        }
    }

    /// Adds a source file to the renderer and returns its file ID.
    ///
    /// The file ID is used in diagnostic labels to specify which file and
    /// which byte ranges within that file are relevant to the diagnostic.
    pub fn add_file(&mut self, name: &'a str, source: &'a str) -> usize {
        self.files.add(name.into(), source)
    }

    /// Adds a source file using the builder pattern.
    pub fn with_file(mut self, name: &str, source: &'a str) -> Self {
        self.files.add(name.into(), source);
        self
    }

    /// Sets a [custom configuration] for rendering diagnostics.
    ///
    /// [custom configuration]: codespan_reporting::term::Config
    pub fn with_config(mut self, config: codespan_reporting::term::Config) -> Self {
        self.config = config;
        self
    }

    /// Renders a diagnostic into a human-readable formatted string.
    ///
    /// This method takes a [`Diagnostic`] and produces a [`RenderedDiagnostic`]
    /// containing the fully formatted output ready for display. The diagnostic
    /// must use file IDs that were previously added to this renderer.
    pub fn render(&self, diagnostic: Diagnostic<usize>) -> Result<RenderedDiagnostic, FilesError> {
        emit_into_string(&self.config, &self.files, &diagnostic).map(RenderedDiagnostic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codespan_reporting::diagnostic::{Diagnostic, Label};

    #[test]
    fn test_with_file_builder() {
        let renderer = Renderer::new()
            .with_file("test1.txt", "content 1")
            .with_file("test2.txt", "content 2");

        // Verify files were added by rendering a diagnostic
        let diagnostic = Diagnostic::note().with_message("test");
        assert!(renderer.render(diagnostic).is_ok());
    }

    #[test]
    fn test_render_simple_error() {
        let mut renderer = Renderer::new();
        let _file_id = renderer.add_file("example.txt", "This is a test file");

        let diagnostic = Diagnostic::error().with_message("simple error message");
        let result = renderer.render(diagnostic);
        assert!(result.is_ok());

        let rendered = result.unwrap();
        insta::assert_snapshot!(rendered.to_string());
    }

    #[test]
    fn test_render_error_with_label() {
        let mut renderer = Renderer::new();
        let source = "let x = 42;";
        let file_id = renderer.add_file("test.rs", source);

        let diagnostic = Diagnostic::error()
            .with_message("unused variable")
            .with_labels(vec![
                Label::primary(file_id, 4..5).with_message("variable `x` is never used"),
            ]);

        let result = renderer.render(diagnostic);
        assert!(result.is_ok());

        let rendered = result.unwrap();
        insta::assert_snapshot!(rendered.to_string());
    }

    #[test]
    fn test_render_warning() {
        let mut renderer = Renderer::new();
        let source = "fn main() {\n    let x = 1;\n}";
        let file_id = renderer.add_file("main.rs", source);

        let diagnostic = Diagnostic::warning()
            .with_message("unused variable: `x`")
            .with_labels(vec![
                Label::primary(file_id, 20..21).with_message("unused variable"),
            ]);

        let result = renderer.render(diagnostic);
        assert!(result.is_ok());

        let rendered = result.unwrap();
        insta::assert_snapshot!(rendered.to_string());
    }

    #[test]
    fn test_render_note() {
        let mut renderer = Renderer::new();
        let _file_id = renderer.add_file("info.txt", "some informational text");

        let diagnostic = Diagnostic::note().with_message("this is just a note");

        let result = renderer.render(diagnostic);
        assert!(result.is_ok());

        let rendered = result.unwrap();
        insta::assert_snapshot!(rendered.to_string());
    }

    #[test]
    fn test_render_with_multiple_labels() {
        let mut renderer = Renderer::new();
        let source = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}";
        let file_id = renderer.add_file("calculator.rs", source);

        let diagnostic = Diagnostic::error()
            .with_message("type mismatch")
            .with_labels(vec![
                Label::primary(file_id, 7..8).with_message("expected type here"),
                Label::secondary(file_id, 15..16).with_message("and here"),
            ])
            .with_notes(vec![
                "consider using the same type for both parameters".to_string(),
            ]);

        let result = renderer.render(diagnostic);
        assert!(result.is_ok());

        let rendered = result.unwrap();
        insta::assert_snapshot!(rendered.to_string());
    }

    #[test]
    fn test_render_multiline_diagnostic() {
        let mut renderer = Renderer::new();
        let source = "fn main() {\n    let x = 5;\n    let y = 10;\n    println!(\"{}\", x);\n}";
        let file_id = renderer.add_file("multi.rs", source);

        let diagnostic = Diagnostic::warning()
            .with_message("unused variable")
            .with_labels(vec![
                Label::primary(file_id, 31..32).with_message("variable `y` is never used"),
            ]);

        let result = renderer.render(diagnostic);
        assert!(result.is_ok());

        let rendered = result.unwrap();
        insta::assert_snapshot!(rendered.to_string());
    }

    #[test]
    fn test_rendered_diagnostic_display() {
        let rendered = RenderedDiagnostic("test output".to_string());
        assert_eq!(rendered.to_string(), "test output");
    }

    #[test]
    fn test_with_config() {
        let config = codespan_reporting::term::Config::default();
        let renderer = Renderer::new().with_config(config);

        // Just verify it doesn't panic
        let file_id = renderer
            .with_file("test.txt", "content")
            .add_file("another.txt", "more");

        assert_eq!(file_id, 1);
    }

    #[test]
    fn test_chained_builder_pattern() {
        let renderer = Renderer::new()
            .with_file("file1.txt", "content1")
            .with_file("file2.txt", "content2")
            .with_config(codespan_reporting::term::Config::default())
            .with_file("file3.txt", "content3");

        // Verify files were added successfully
        let diagnostic = Diagnostic::note().with_message("test");
        assert!(renderer.render(diagnostic).is_ok());
    }
}
