mod pause_when;
mod waiting;

use core::{
    future::Future,
    mem,
    ops::{Add, Deref},
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::Poll,
};

use alloc::{boxed::Box, collections::BinaryHeap};
use hashbrown::HashMap;
use parking_lot::Mutex;

use waiting::Waiting;

use self::pause_when::PauseWhen;

#[macro_export]
macro_rules! async_fn {
    (|$($argv:ident$(:$argt:ty)?),*|$body:expr) => {
        move |$($argv$(:$argt)?),*| {
            alloc::boxed::Box::pin(async move {$body})
        }
    };
}

pub trait Time: Ord + Clone {}
impl<T> Time for T where Self: Ord + Clone {}

pub struct Timewrap<T>
where
    T: Time,
{
    data: Mutex<TimewrapData<T>>,
    is_by_time: AtomicBool,
    current_id: AtomicUsize,
}

struct TimewrapData<T>
where
    T: Time,
{
    current: T,
    tasks: HashMap<usize, Pin<Box<dyn Future<Output = ()> + Send>>>,
    waitings: BinaryHeap<Waiting<T>>,
}

#[derive(Clone, Copy)]
pub struct TimewrapHandle<'a, T>
where
    T: Time,
{
    target: &'a Timewrap<T>,
}

impl<'a, T> Deref for TimewrapHandle<'a, T>
where
    T: Time,
{
    type Target = Timewrap<T>;
    fn deref(&self) -> &Self::Target {
        self.target
    }
}

fn as_ptr<T: ?Sized>(v: &T) -> *const T {
    v as *const T
}

impl<T> Default for Timewrap<T>
where
    T: Time + Default,
{
    fn default() -> Self {
        Self::new_with_time(Default::default())
    }
}

impl<T> Timewrap<T>
where
    T: Time + Default,
{
    pub fn new() -> Self {
        Default::default()
    }
}

impl<T> Timewrap<T>
where
    T: Time,
{
    pub fn new_with_time(time: T) -> Self {
        Self {
            data: Mutex::new(TimewrapData {
                current: time,
                tasks: HashMap::new(),
                waitings: BinaryHeap::new(),
            }),
            is_by_time: AtomicBool::new(false),
            current_id: AtomicUsize::new(0),
        }
    }

    pub fn spawn<F>(&self, f: F)
    where
        F: for<'a> FnOnce(TimewrapHandle<'a, T>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>,
    {
        let f = f(TimewrapHandle { target: self });
        let id = as_ptr(f.as_ref().get_ref()) as *const () as usize;
        let mut this = self.data.lock();
        let this = &mut *this;
        this.tasks.insert(id, unsafe { mem::transmute(f) });
        this.waitings.push(Waiting {
            id,
            time: this.current.clone(),
        })
    }

    pub fn spawn_on<F>(&self, time: T, f: F)
    where
        F: for<'a> FnOnce(TimewrapHandle<'a, T>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>,
    {
        let f = f(TimewrapHandle { target: self });
        let id = as_ptr(f.as_ref().get_ref()) as *const () as usize;
        let mut this = self.data.lock();
        this.tasks.insert(id, unsafe { mem::transmute(f) });
        this.waitings.push(Waiting { id, time })
    }

    pub async fn drive(&mut self, time: T) {
        let mut this = self.data.lock();
        while let Some(w) = this.waitings.peek() {
            if w.time > time {
                break;
            }
            let w = this.waitings.pop().unwrap();
            this.current = w.time;
            let mut task = this.tasks.remove(&w.id).unwrap();
            drop(this);
            self.is_by_time.store(false, Ordering::Relaxed);
            self.current_id.store(w.id, Ordering::Relaxed);
            let pause =
                PauseWhen(&mut task, || self.is_by_time.swap(false, Ordering::Relaxed)).await;
            this = self.data.lock();
            if pause.is_pending() {
                this.tasks.insert(w.id, task);
            };
        }
        this.current = time;
    }

    #[cfg(feature = "drive_block")]
    pub fn drive_block(&mut self, time: T) {
        futures::executor::block_on(self.drive(time));
    }
}

impl<'a, T> TimewrapHandle<'a, T>
where
    T: Time,
{
    pub fn at(self, time: T) -> At<'a, T> {
        At {
            timewrap: self.target,
            time,
        }
    }
}

impl<'a, T> TimewrapHandle<'a, T>
where
    T: Time + Add<Output = T>,
{
    pub fn delay(self, time: T) -> At<'a, T> {
        At {
            timewrap: self.target,
            time: time + self.target.data.lock().current.clone(),
        }
    }
}

#[must_use]
pub struct At<'a, T>
where
    T: Time,
{
    timewrap: &'a Timewrap<T>,
    time: T,
}

impl<'a, T> Future for At<'a, T>
where
    T: Time,
{
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut core::task::Context) -> core::task::Poll<()> {
        let mut this = self.timewrap.data.lock();
        let this = &mut this;
        if this.current < self.time {
            self.timewrap.is_by_time.store(true, Ordering::Relaxed);
            this.waitings.push(Waiting {
                id: self.timewrap.current_id.load(Ordering::Relaxed),
                time: self.time.clone(),
            });
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}
