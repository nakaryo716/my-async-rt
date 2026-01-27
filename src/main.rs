use std::{
    cell::RefCell,
    collections::VecDeque,
    marker::PhantomData,
    pin::{Pin, pin},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake},
    thread::Thread,
};

use crate::count_fut::CounterFut;

pub mod count_fut;

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

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}

pub struct JoinHandle<T> {
    _phantom: PhantomData<T>,
}

pub struct Handle {
    scheduler: Weak<CurrentThreadScheduler>,
}

pub enum RtError {
    ShutDown,
}

/*
 *
 * ===== impl Runtime =====
 *
 */
impl Runtime {
    fn new() -> Self {
        let scheduler = Rc::new(CurrentThreadScheduler {
            core: Rc::new(Core::new()),
        });
        let handle = Handle {
            scheduler: Rc::downgrade(&scheduler),
        };
        Self { scheduler, handle }
    }

    fn spawn<Fut>(&self, fut: Fut) -> Result<(), RtError>
    where
        Fut: Future<Output = ()> + 'static,
    {
        let task = Task {
            future: Box::pin(fut),
        };

        let scheduler = match self.handle.scheduler.upgrade() {
            Some(scheduler) => scheduler,
            None => return Err(RtError::ShutDown),
        };
        let mut task_queue = scheduler.core.task.borrow_mut();
        task_queue.push_back(task);
        Ok(())
    }

    fn block_on<Fut>(&self, fut: Fut) -> Fut::Output
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
                        if task.run(&mut cx).is_pending() {
                            queue.push_back(task);
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

fn main() {
    let rt = Runtime::new();

    let future = async {
        let _ = rt.spawn(async move {
            let count_fut = CounterFut::new(4);
            println!("task1 {}", count_fut.await);
        });

        let _ = rt.spawn(async move {
            let count_fut2 = CounterFut::new(2);
            println!("task2 {}", count_fut2.await);
        });

        let count_fut3 = CounterFut::new(1);
        let val = count_fut3.await;
        println!("block on {}", val)
    };

    rt.block_on(future);
}
