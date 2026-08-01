use std::fmt;
use tokio::sync::watch;

/// A notification object that allows for graceful shutdown coordination across
/// all Eden's concurrent services.
///
/// Cloning this is cheap as it stores [`tokio::sync::watch`] internally.
///
/// # Example
/// ```no_run
/// # use eden_signals::ShutdownSignal;
/// # use tokio::time::Duration;
/// #
/// # #[tokio::main]
/// # async fn main() {
/// let signal = ShutdownSignal::new();
/// # let signal_1 = signal.clone();
/// tokio::spawn(async {
/// #   let signal = signal_1;
///     tokio::time::sleep(Duration::from_secs(1)).await;
///     signal.initiate();
/// });
///
/// signal.wait().await;
/// println!("Requested graceful shutdown!");
/// # }
/// ```
#[derive(Clone)]
pub struct ShutdownSignal {
    inner: watch::Sender<bool>,
}

impl ShutdownSignal {
    #[expect(
        clippy::new_without_default,
        reason = "ShutdownSignal is not configurable"
    )]
    #[must_use]
    pub fn new() -> Self {
        let (tx, _) = watch::channel(false);
        Self { inner: tx }
    }

    /// Races a fallible future against shutdown, returning `Ok(None)` on cancellation.
    pub async fn try_run_or_cancel<T, E, F>(&self, future: F) -> Result<Option<T>, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        match self.run_or_cancel(future).await {
            Some(Ok(okay)) => Ok(Some(okay)),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
    }

    /// Races a future against shutdown, returning `None` if shutdown fires first.
    pub async fn run_or_cancel<F>(&self, future: F) -> Option<F::Output>
    where
        F: Future,
    {
        let shutdown = Box::pin(self.wait());
        let future = Box::pin(future);
        tokio::select! {
            _ = shutdown => None,
            output = future => Some(output)
        }
    }

    /// Waits until shutdown is initiated.
    pub async fn wait(&self) {
        let mut rx = self.inner.subscribe();
        _ = rx.wait_for(|initiated| *initiated).await;
    }

    /// Broadcasts shutdown to all waiters.
    pub fn initiate(&self) {
        let _ = self.inner.send_replace(true);
    }

    pub fn is_initiated(&self) -> bool {
        *self.inner.borrow()
    }
}

impl fmt::Debug for ShutdownSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShutdownSignal")
            .field("initiated", &self.is_initiated())
            .finish()
    }
}

impl PartialEq for ShutdownSignal {
    fn eq(&self, other: &Self) -> bool {
        self.inner.same_channel(&other.inner)
    }
}

impl Eq for ShutdownSignal {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn new_signal_is_not_initiated() {
        let signal = ShutdownSignal::new();
        assert!(!signal.is_initiated());
    }

    #[tokio::test]
    async fn initiate_sets_state() {
        let signal = ShutdownSignal::new();
        signal.initiate();
        assert!(signal.is_initiated());
    }

    #[tokio::test]
    async fn wait_unblocks_on_initiate() {
        let signal = ShutdownSignal::new();
        let cloned = signal.clone();

        let handle = tokio::spawn(async move {
            cloned.wait().await;
        });

        tokio::task::yield_now().await;
        assert!(!handle.is_finished());

        signal.initiate();

        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("waiter timed out")
            .expect("waiter panicked");
    }

    #[tokio::test]
    async fn run_or_cancel_completes_without_shutdown() {
        let signal = ShutdownSignal::new();
        assert_eq!(signal.run_or_cancel(async { 42 }).await, Some(42));
    }

    #[tokio::test]
    async fn run_or_cancel_returns_none_when_already_initiated() {
        let signal = ShutdownSignal::new();
        signal.initiate();

        let result = signal
            .run_or_cancel(async {
                tokio::time::sleep(Duration::from_secs(10)).await;
            })
            .await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn run_or_cancel_cancels_pending_future() {
        let signal = ShutdownSignal::new();
        let cloned = signal.clone();

        let handle = tokio::spawn(async move {
            cloned
                .run_or_cancel(async {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    42
                })
                .await
        });

        tokio::task::yield_now().await;
        signal.initiate();

        let output = tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("timed out")
            .expect("panicked");

        assert_eq!(output, None);
    }

    #[tokio::test]
    async fn try_run_or_cancel_forwards_ok() {
        let signal = ShutdownSignal::new();
        let res: Result<Option<i32>, &str> = signal.try_run_or_cancel(async { Ok(100) }).await;
        assert_eq!(res, Ok(Some(100)));
    }

    #[tokio::test]
    async fn try_run_or_cancel_forwards_err() {
        let signal = ShutdownSignal::new();
        let res: Result<Option<i32>, &str> = signal.try_run_or_cancel(async { Err("fail") }).await;
        assert_eq!(res, Err("fail"));
    }

    #[tokio::test]
    async fn try_run_or_cancel_returns_ok_none_on_cancellation() {
        let signal = ShutdownSignal::new();
        signal.initiate();

        let res: Result<Option<i32>, &str> = signal
            .try_run_or_cancel(async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(100)
            })
            .await;

        assert_eq!(res, Ok(None));
    }

    #[tokio::test]
    async fn same_signal_is_equal() {
        let signal = ShutdownSignal::new();
        assert_eq!(signal, signal);
    }

    #[tokio::test]
    async fn different_signals_are_not_equal() {
        let a = ShutdownSignal::new();
        let b = ShutdownSignal::new();
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn debug_reflects_state() {
        let signal = ShutdownSignal::new();
        assert!(format!("{signal:?}").contains("initiated: false"));

        signal.initiate();
        assert!(format!("{signal:?}").contains("initiated: true"));
    }
}
