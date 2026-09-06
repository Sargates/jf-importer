use std::cell::RefCell;
use std::boxed::Box;
use std::pin::Pin;

use tokio::{self, spawn};
use tokio::sync::oneshot;
use futures::{Future, FutureExt};

/// "Omnisync" is a shitty, pretentious term that I came up with
/// to literally just imply that this "task" can be utilized from
/// a synchronous or an asynchronous context.
///
/// It requires `tokio` as it utilizes `tokio::spawn` and message
/// passing to return owned data from the provided future.
pub struct OmnisyncTask<T> {
    inner: Option<T>,
    rx: oneshot::Receiver<T>,
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
            rx,
        }
    }

    pub fn is_some(&mut self) -> bool {
        self.try_take_inner();
        self.inner.is_some()
    }
    pub fn take(&mut self) -> Option<T> {
        self.try_take_inner();
        self.inner.take()
    }
    pub fn as_ref(&mut self) -> Option<&T> {
        self.try_take_inner();
        self.inner.as_ref()
    }
    pub fn as_mut(&mut self) -> Option<&mut T> {
        self.try_take_inner();
        self.inner.as_mut()
    }

    fn try_take_inner(&mut self) {
        if let Some(out) = self.rx.try_recv().ok() {
            self.inner = Some(out);
            return
        }
    }
}
