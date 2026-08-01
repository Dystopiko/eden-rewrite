//! # `erased-report`
//!
//! Type-erased wrapper for [`error_stack::Report`] that erases the generic context type parameter.
//!
//! [`ErasedReport`] enables storing error reports without needing to know or preserve the static
//! context type `C`, while keeping the internal frame chain, attachments, and downcasting
//! functionality fully functional.
//!
//! ## Example
//! ```rust
//! # use std::{fs, path::Path};
//! # use erased_report::{EraseReportExt, ErasedReport};
//! # use error_stack::ResultExt;
//! # pub type Config = String;
//! # #[derive(Debug)] struct ParseConfigError;
//! # impl ParseConfigError { pub fn new() -> Self { Self } }
//! # impl std::fmt::Display for ParseConfigError {
//! #     fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//! #         fmt.write_str("could not parse configuration file")
//! #     }
//! # }
//! # impl core::error::Error for ParseConfigError {}
//! # #[derive(Debug, PartialEq)]
//! struct Suggestion(&'static str);
//!
//! fn parse_config(path: impl AsRef<Path>) -> Result<Config, ErasedReport> {
//!     let path = path.as_ref();
//!
//!     let content = fs::read_to_string(path)
//!         .change_context(ParseConfigError::new())
//!         .attach_opaque(Suggestion("use a file you can read next time!"))
//!         .attach_with(|| format!("could not read file {path:?}"))
//!         .erase_report()?;
//!
//!     Ok(content)
//! }
//! ```
//!
//! ## Safety
//!
//! Type erasure is achieved by transmuting a [`Report<C>`] into [`Report<()>`].
//!
//! Across all context types `C`, `Report<C>` maintains an identical memory layout. The context type
//! parameter is solely used as a `PhantomData<fn() -> *const C>` marker, while the underlying error
//! frame tree is opaque and layout-compatible across all `Report` instances.

use core::fmt;
use error_stack::Report;
use std::{error::Error, ops::ControlFlow};

/// A type-erased wrapper for [`error_stack::Report`] that erases the context type `C`.
#[must_use]
pub struct ErasedReport {
    report: Report<()>,
}

impl ErasedReport {
    #[expect(
        unsafe_code,
        reason = "read Safety section in crate-level documentation."
    )]
    #[track_caller]
    pub fn new<C>(report: Report<C>) -> Self
    where
        C: Error + Send + Sync + 'static,
    {
        // SAFETY:
        // Read Safety section found in root crate's documentation.
        //
        // The transmuted report must never invoke `current_context()`,
        // as the erased context type parameter is no longer available.
        let report: Report<()> = unsafe { std::mem::transmute(report) };
        Self { report }
    }

    #[track_caller]
    pub fn new_from<C>(context: C) -> Self
    where
        C: Error + Send + Sync + 'static,
    {
        Self::new(Report::new(context))
    }
}

impl ErasedReport {
    /// Wrapper of [`Report::downcast_ref`].
    #[must_use]
    pub fn downcast_ref<C>(&self) -> Option<&C>
    where
        C: Send + Sync + 'static,
    {
        self.report.downcast_ref()
    }

    /// Wrapper of [`Report::downcast_mut`].
    #[must_use]
    pub fn downcast_mut<C>(&mut self) -> Option<&mut C>
    where
        C: Send + Sync + 'static,
    {
        self.report.downcast_mut()
    }

    /// Wrapper of [`Report::into_error`].
    #[must_use]
    pub fn into_error(self) -> impl Error + Send + Sync + 'static {
        self.report.into_error()
    }

    /// Returns this `Report` as an [`Error`].
    #[must_use]
    pub fn as_error(&self) -> &(impl Error + Send + Sync + 'static) {
        self.report.as_error()
    }

    /// Wrapper of [`Report::frames`].
    pub fn frames(&self) -> error_stack::iter::Frames<'_> {
        self.report.frames()
    }

    /// Wrapper of [`Report::frames_mut`].
    pub fn frames_mut(
        &mut self,
        visitor: impl FnMut(&mut error_stack::Frame) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        self.report.frames_mut(visitor)
    }

    /// Wrapper of [`Report::contains`].
    #[must_use]
    pub fn contains<T: Send + Sync + 'static>(&self) -> bool {
        self.report.contains::<T>()
    }
}

impl ErasedReport {
    /// Wrapper of [`Report::attach`].
    pub fn attach<A>(self, attachment: A) -> Self
    where
        A: error_stack::Attachment,
    {
        Self {
            report: self.report.attach(attachment),
        }
    }

    /// Wrapper of [`Report::attach_opaque`].
    #[track_caller]
    pub fn attach_opaque<A>(self, attachment: A) -> Self
    where
        A: error_stack::OpaqueAttachment,
    {
        Self {
            report: self.report.attach_opaque(attachment),
        }
    }

    /// Pushes a new context frame onto the report while maintaining
    /// type erasure (`ErasedReport`).
    ///
    /// Use [`change_context`](Self::change_context) to convert back
    /// into a typed [`Report<C>`].
    #[track_caller]
    pub fn push_context<T>(self, context: T) -> Self
    where
        T: Error + Send + Sync + 'static,
    {
        // SAFETY: Read Safety section found in root crate's documentation.
        Self::new(self.report.change_context(context))
    }

    /// Pushes a new context frame onto the report and converts it
    /// into a typed [`Report<C>`].
    ///
    /// Use [`push_context`](Self::push_context) to retain the
    /// type-erased [`ErasedReport`].
    #[track_caller]
    pub fn change_context<T>(self, context: T) -> Report<T>
    where
        T: Error + Send + Sync + 'static,
    {
        // SAFETY: Read Safety section found in root crate's documentation.
        self.report.change_context(context)
    }
}

impl<C: Error + Send + Sync + 'static> From<C> for ErasedReport {
    #[track_caller]
    #[inline(always)]
    fn from(value: C) -> Self {
        ErasedReport::new_from(value)
    }
}

impl fmt::Debug for ErasedReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.report, f)
    }
}

impl fmt::Display for ErasedReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.report, f)
    }
}

/// Extension trait for converting a `Result<T, C>` into `Result<T, ErasedReport>`,
/// erasing the context type parameter from the error.
///
/// This is useful when results of varying `C` types need to be stored, returned,
/// or passed through a layer that should not depend on a specific context type.
pub trait IntoErasedReportExt<T> {
    /// Converts a `Result<T, C>` into `Result<T, ErasedReport>`, erasing the context type
    /// parameter from the error.
    fn erase_report(self) -> Result<T, ErasedReport>;
}

impl<T, C> IntoErasedReportExt<T> for Result<T, C>
where
    C: Error + Send + Sync + 'static,
{
    #[track_caller]
    #[inline(always)]
    fn erase_report(self) -> Result<T, ErasedReport> {
        self.map_err(ErasedReport::new_from)
    }
}

/// Extension trait for converting a `Result<T, Report<C>>` into `Result<T, ErasedReport>`,
/// erasing the context type parameter from the error.
///
/// This is useful when results of varying `Report<C>` types need to be stored,
/// returned, or passed through a layer that should not depend on a specific context type.
pub trait EraseReportExt<T> {
    /// Converts a `Result<T, Report<C>>` into `Result<T, ErasedReport>`, erasing the context type
    /// parameter from the error.
    fn erase_report(self) -> Result<T, ErasedReport>;
}

impl<T, C> EraseReportExt<T> for Result<T, Report<C>>
where
    C: Error + Send + Sync + 'static,
{
    #[track_caller]
    #[inline(always)]
    fn erase_report(self) -> Result<T, ErasedReport> {
        self.map_err(ErasedReport::new)
    }
}

#[cfg(test)]
mod tests {
    use error_stack::{Report, ResultExt};
    use std::hint::black_box;
    use thiserror::Error;

    use crate::{EraseReportExt, ErasedReport, IntoErasedReportExt};

    #[derive(Debug, Error)]
    #[error("custom error")]
    struct CustomError;

    #[derive(Debug, Error)]
    #[error("could not parse configuration file")]
    struct ParseConfigError;

    #[expect(dead_code, reason = "tested via downcast_ref")]
    #[derive(Debug)]
    struct Suggestion(&'static str);

    fn produce_report() -> ErasedReport {
        let error = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not find specified file",
        );

        Err::<(), _>(error)
            .change_context(ParseConfigError)
            .attach_opaque(Suggestion("use a file you can read next time!"))
            .attach_with(|| "hopefully it should not throw SIGFAULT to us")
            .erase_report()
            .unwrap_err()
    }

    #[test]
    fn can_use_downcast_ref() {
        let report = produce_report();
        let suggestion = report.downcast_ref::<Suggestion>();
        assert!(suggestion.is_some());

        let report = produce_report();
        let error = report.downcast_ref::<ParseConfigError>();
        assert!(error.is_some());
    }

    #[test]
    fn should_not_emit_segfault_in_debug() {
        black_box(format!("{:?}", produce_report()));
        black_box(format!("{:#?}", produce_report()));
    }

    #[test]
    fn should_not_emit_segfault_in_display() {
        black_box(format!("{}", produce_report()));
    }

    #[test]
    fn should_convert_result_context_into_erased_report() {
        let res: Result<i32, CustomError> = Err(CustomError);
        let erased: Result<i32, ErasedReport> = res.erase_report();
        assert!(erased.is_err());
        assert!(erased.unwrap_err().contains::<CustomError>());
    }

    #[test]
    fn should_erase_report_type_from_result() {
        let report = Report::new(CustomError);
        let res: Result<i32, Report<CustomError>> = Err(report);
        let erased: Result<i32, ErasedReport> = res.erase_report();
        assert!(erased.is_err());
        assert!(erased.unwrap_err().contains::<CustomError>());
    }
}
