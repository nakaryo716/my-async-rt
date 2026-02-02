use std::{
    cell::RefCell,
    collections::VecDeque,
    pin::{Pin, pin},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    thread::Thread,
};

pub struct Runtime {
    scheduler: Rc<CurrentThreadScheduler>,
    handle: Handle,
}

pub struct CurrentThreadScheduler {
    core: Rc<Core>,
}

struct Core {
    task: RefCell<VecDeque<Task>>,
}

pub struct Handle {
    scheduler: Weak<CurrentThreadScheduler>,
}

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
    waker: Rc<RefCell<Option<Waker>>>,
}

pub struct JoinHandle<T> {
    data: Rc<RefCell<Option<T>>>,
    waker: Rc<RefCell<Option<Waker>>>,
}

#[derive(Debug)]
pub enum RtError {
    ShutDown,
}

/*
 *
 * ===== impl Runtime =====
 *
 */

impl Runtime {
    pub fn new() -> Self {
        let scheduler = Rc::new(CurrentThreadScheduler {
            core: Rc::new(Core::new()),
        });
        let handle = Handle {
            scheduler: Rc::downgrade(&scheduler),
        };
        Self { scheduler, handle }
    }

    pub fn spawn<Fut, T>(&self, fut: Fut) -> Result<JoinHandle<T>, RtError>
    where
        Fut: Future<Output = T> + 'static,
        T: Send + 'static,
    {
        let data = Rc::new(RefCell::new(None));
        let data_c = data.clone();

        let waker = Rc::new(RefCell::new(None));
        let waker_c = waker.clone();

        let task = Task {
            future: Box::pin(async move {
                let val = fut.await;
                *data_c.borrow_mut() = Some(val);
            }),
            waker: waker_c,
        };

        let handle = JoinHandle { data, waker };

        let scheduler = match self.handle.scheduler.upgrade() {
            Some(scheduler) => scheduler,
            None => return Err(RtError::ShutDown),
        };
        let mut task_queue = scheduler.core.task.borrow_mut();
        task_queue.push_back(task);
        Ok(handle)
    }

    pub fn block_on<Fut>(&self, fut: Fut) -> Fut::Output
    where
        Fut: Future,
    {
        // create runtime context
        let thread = std::thread::current();
        let notified = Arc::new(AtomicBool::new(false));
        let waker = Arc::new(ThreadWaker {
            thread,
            notified: notified.clone(),
        })
        .into();
        let mut cx = Context::from_waker(&waker);

        let mut fut = pin!(fut);

        'outer: loop {
            // poll given future
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(v) => return v,
                Poll::Pending => {}
            }

            // poll enqueued task
            // if empty park
            'inner: loop {
                let mut queue = self.scheduler.core.task.borrow_mut();
                match queue.pop_front() {
                    Some(mut task) => {
                        match task.run(&mut cx) {
                            Poll::Ready(_) => {
                                if let Some(waker) = task.waker.borrow_mut().take() {
                                    waker.wake();
                                }
                            }
                            Poll::Pending => queue.push_back(task),
                        }
                        continue 'inner;
                    }
                    None => {
                        if notified.load(Ordering::Acquire) {
                            continue 'outer;
                        }
                        std::thread::park();
                        continue 'outer;
                    }
                };
            }
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/*
 *
 * ===== impl Core =====
 *
 */

impl Core {
    fn new() -> Self {
        Self {
            task: RefCell::new(VecDeque::default()),
        }
    }
}

/*
 *
 * ===== impl Task =====
 *
 */

impl Task {
    fn run(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.future.as_mut().poll(cx)
    }
}

/*
 *
 * ===== Waker =====
 *
 */

struct ThreadWaker {
    thread: Thread,
    notified: Arc<AtomicBool>,
}

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        if self
            .notified
            .compare_exchange(false, true, Ordering::Release, Ordering::Relaxed)
            .is_ok()
        {
            self.thread.unpark();
        }
    }
}

/*
 *
 * ===== impl JoinHandle =====
 *
 */

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.data.borrow_mut().take() {
            Some(v) => Poll::Ready(v),
            None => {
                *self.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}
