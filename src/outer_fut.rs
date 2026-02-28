use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

pub struct OuterFut {
    inner: Arc<Mutex<Inner>>,
}

pub struct OuterWaker {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    data: bool,
    waker: Option<Waker>,
}

impl OuterFut {
    pub fn new() -> (Self, OuterWaker) {
        let inner = Arc::new(Mutex::new(Inner {
            data: false,
            waker: None,
        }));

        let outer_fut = Self {
            inner: inner.clone(),
        };
        let outer_waker = OuterWaker { inner };

        (outer_fut, outer_waker)
    }
}

impl Future for OuterFut {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.inner.lock().unwrap();

        if inner.data {
            Poll::Ready(())
        } else {
            inner.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl OuterWaker {
    pub fn wake(&self) {
        let mut inner = self.inner.lock().unwrap();

        inner.data = true;

        if let Some(waker) = inner.waker.take() {
            waker.wake();
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{outer_fut::OuterFut, runtime::Runtime};

    #[test]
    fn test_wake() {
        let rt = Runtime::new();

        let (outer_fut, outer_waker) = OuterFut::new();

        let fut = async move {
            outer_fut.await;
        };

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            outer_waker.wake();
        });

        rt.block_on(fut);
    }
}
