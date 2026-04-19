use eden_file_diagnostics::{
    RenderedDiagnostic, Renderer,
    codespan_reporting::diagnostic::{Diagnostic, Label},
};
use std::{fmt, path::Path};
use toml_edit::Document;

pub struct SourceContext<'a> {
    pub source: &'a str,
    pub path: &'a Path,
    pub document: &'a Document<String>,
}

impl<'a> SourceContext<'a> {
    pub fn field_diagnostic_with_doc<M>(
        &self,
        document: &toml_edit::Item,
        path: &[&str],
        message: M,
    ) -> DiagnosticModifier<'a>
    where
        M: fmt::Display,
    {
        let span = path
            .iter()
            .try_fold(document, |item, key| item.get(key))
            .and_then(|item| item.span());

        let path = self.path.to_string_lossy();
        let renderer = Renderer::new().with_file(&path, self.source);

        let mut diagnostic = Diagnostic::error().with_message(message);
        if let Some(span) = span {
            let label = Label::primary(0usize, span);
            diagnostic = diagnostic.with_label(label);
        }

        DiagnosticModifier {
            renderer,
            inner: diagnostic,
        }
    }

    pub fn field_diagnostic<M>(&self, path: &[&str], message: M) -> DiagnosticModifier<'a>
    where
        M: fmt::Display,
    {
        self.field_diagnostic_with_doc(self.document.as_item(), path, message)
    }
}

pub struct DiagnosticModifier<'s> {
    pub renderer: Renderer<'s>,
    pub inner: Diagnostic<usize>,
}

impl<'a> DiagnosticModifier<'a> {
    #[must_use]
    pub fn with_file(self, name: &str, source: &'a str) -> Self {
        Self {
            renderer: self.renderer.with_file(name, source),
            ..self
        }
    }

    #[must_use]
    pub fn with_label(self, label: Label<usize>) -> Self {
        Self {
            inner: self.inner.with_label(label),
            ..self
        }
    }

    #[must_use]
    pub fn with_note(self, note: impl fmt::Display) -> Self {
        Self {
            inner: self.inner.with_note(note),
            ..self
        }
    }

    #[must_use]
    pub fn into_diagnostic(self) -> RenderedDiagnostic {
        self.renderer
            .render(self.inner)
            .expect("rendering should succeed with valid file data")
    }

    pub fn emit(self) -> Result<(), RenderedDiagnostic> {
        Err(self.into_diagnostic())
    }
}
