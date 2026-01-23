use std::{
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Clone)]
pub struct CounterFut {
    curr: usize,
    target: usize,
}

impl CounterFut {
    pub fn new(target: usize) -> Self {
        Self { curr: 0, target }
    }
}

impl Future for CounterFut {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.curr == self.target {
            Poll::Ready(self.curr)
        } else {
            self.curr += 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod test {
    use std::task::{Context, Poll, Waker};

    use crate::count_fut::CounterFut;

    #[test]
    fn test() {
        let cnt_fut = CounterFut::new(3);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(&waker);

        // count 0 -> 1
        let mut fut = Box::pin(cnt_fut);
        let res = fut.as_mut().poll(&mut cx);
        assert!(res.is_pending());

        // count 1 -> 2
        let res = fut.as_mut().poll(&mut cx);
        assert!(res.is_pending());

        // count 2 -> 3
        let res = fut.as_mut().poll(&mut cx);
        assert!(res.is_pending());

        // Ready
        let res = fut.as_mut().poll(&mut cx);
        assert_eq!(res, Poll::Ready(3));
    }
}
