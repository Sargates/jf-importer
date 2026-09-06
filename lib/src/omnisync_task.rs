use std::cell::RefCell;
use std::boxed::Box;
use std::pin::Pin;

use tokio::{self, spawn};
use tokio::sync::oneshot;
use futures::{Future, FutureExt};

/// An async task that can be queried like an option. Uses a
/// `oneshot::channel` to acquire work from the async future
/// that's provided.
///
/// If asynchronous polling is required, `poll` is exposed but it
/// returns an `Option<&mut T>` which is an exclusive reference
/// to the `inner` value. As such, polling after completion is
/// supported, but the lifetime of the `task` must be maintained
/// an proper terminated.
///
/// "Omnisync" is a shitty, pretentious term that I came up with
/// to literally just imply that this "task" can be utilized from
/// a synchronous or an asynchronous context.
/// # Example
/// ```
/// # use jfi::OmnisyncTask;
/// # use tokio;
/// # #[tokio::test(start_paused = true)]
/// # async fn test() {
/// let data = 42;
///                                                                           
/// let mut task = OmnisyncTask::new(async move {
///     tokio::time::sleep(tokio::time::Duration::from_millis(500u64)).await;
///     data
/// });
///                                                                           
/// while let None = task.poll().await {}
///                                                                           
/// let data = task.take().unwrap();
///                                                                           
/// assert_eq!(data, 42);
/// # }
/// ```
pub struct OmnisyncTask<T> {
    inner: Option<T>,
    rx: Option<oneshot::Receiver<T>>,
}

impl<T> OmnisyncTask<T>
where
    // `T: 'static` DOES NOT mean that T lives for the `'static` lifetime,
    // simply that `T` is shorter lived than the `'static` lifetime--which
    // is the space that tokio operates within.
    // I forgot lifetime syntax meanings.
    // ref: https://users.rust-lang.org/t/satisfying-tokio-spawn-static-lifetime-requirement/78773/2
    T: Send + 'static
{
    /// Create a new `OmnisyncTask`
    pub fn new<F>(future: F) -> Self
    where 
        F: Future<Output = T> + Send + 'static
    {
        let (tx, rx) = oneshot::channel();

        tokio::task::spawn(async move {
            tx.send(future.await);
        });

        Self {
            inner: None,
            rx: Some(rx),
        }
    }

    /// Check if the inner `Option<T>` is `Some`
    pub fn is_some(&mut self) -> bool {
        self.try_take_inner();
        self.inner.is_some()
    }

    /// Take the value contained in the inner
    pub fn take(&mut self) -> Option<T> {
        self.try_take_inner();
        self.inner.take()
    }

    /// Acquire a shared reference to the inner
    pub fn as_ref(&mut self) -> Option<&T> {
        self.try_take_inner();
        self.inner.as_ref()
    }

    /// Acquire an exclusive reference to the inner
    pub fn as_mut(&mut self) -> Option<&mut T> {
        self.try_take_inner();
        self.inner.as_mut()
    }

    /// Polls the receiver and extracts the payload into the inner.
    ///
    /// Returns an option of an exclusive reference to the `inner` value.
    /// This allows for polling after completion and, as a result, requires
    /// that the lifetime of the `task` be maintained externally.
    /// 
    /// **This method is not cancel-safe.** Don't use with `tokio::select!` or
    /// the receiver can be dropped and the payload will be lost. If you
    /// want non-blocking, you will want `as_mut` with additional non-blocking
    /// logic.
    pub async fn poll(&mut self) -> Option<&mut T> {
        let rx = self.rx.take();
        if let Some(rx) = rx {
            if let Ok(data) = rx.await {
                self.inner = Some(data);
            }
        } else {
            self.rx = rx;
        }
        self.inner.as_mut()
    }

    /// Try to extract from the receiver
    fn try_take_inner(&mut self) {
        if let Some(ref mut rx) = self.rx &&
           let Some(out) = rx.try_recv().ok() {
            self.inner = Some(out);
        }
    }
}
