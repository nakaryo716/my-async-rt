use std::{
    cell::RefCell,
    collections::VecDeque,
    marker::PhantomData,
    pin::pin,
    rc::Rc,
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

pub struct Task {}

pub struct JoinHandle<T> {
    _phantom: PhantomData<T>,
}

pub struct Handle {
    scheduler: Rc<CurrentThreadScheduler>,
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
            scheduler: scheduler.clone(),
        };
        Self { scheduler, handle }
    }

    fn spawn<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
    where
        Fut: Future,
    {
        // pin future
        let fut = Box::pin(fut);
        // convert to task
        // schedule task(push to queue)
        todo!()
    }

    fn block_on<Fut>(&self, fut: Fut) -> Fut::Output
    where
        Fut: Future + Clone,
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
                match self.scheduler.core.task.borrow_mut().pop_front() {
                    Some(task) => {
                        task.run();
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
    fn run(&self) {
        todo!()
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
    let count_fut = CounterFut::new(4);

    let result = rt.block_on(count_fut);
    println!("block_on result: {}", result);
}
