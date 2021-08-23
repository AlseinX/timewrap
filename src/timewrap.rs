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
    (||$body:expr) => {
        async_fn!(| |$body);
    };
}

pub trait Time: Ord + Clone {}
impl<T> Time for T where Self: Ord + Clone {}

pub struct Timewrap<T, S = ()>
where
    T: Time,
{
    data: Mutex<TimewrapData<T>>,
    state: S,
    is_by_time: AtomicBool,
    current_id: AtomicUsize,
    #[cfg(feature = "drive_shared")]
    shared_lock: futures::lock::Mutex<()>,
}

impl<T, S> Deref for Timewrap<T, S>
where
    T: Time,
{
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

struct TimewrapData<T>
where
    T: Time,
{
    current: T,
    tasks: HashMap<usize, Pin<Box<dyn Future<Output = ()> + Send>>>,
    waitings: BinaryHeap<Waiting<T>>,
}

pub struct TimewrapHandle<'a, T, S = ()>
where
    T: Time,
{
    target: &'a Timewrap<T, S>,
}

impl<'a, T, S> Clone for TimewrapHandle<'a, T, S>
where
    T: Time,
{
    fn clone(&self) -> Self {
        Self {
            target: self.target,
        }
    }
}

impl<'a, T, S> Copy for TimewrapHandle<'a, T, S> where T: Time {}

impl<'a, T, S> Deref for TimewrapHandle<'a, T, S>
where
    T: Time,
{
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.target.state
    }
}

fn as_ptr<T: ?Sized>(v: &T) -> *const T {
    v as *const T
}

impl<T, S> Default for Timewrap<T, S>
where
    T: Time + Default,
    S: Default,
{
    fn default() -> Self {
        Self::new_with_time(Default::default())
    }
}

impl<T, S> Timewrap<T, S>
where
    T: Time + Default,
    S: Default,
{
    pub fn new() -> Self {
        Default::default()
    }
}

impl<T, S> Timewrap<T, S>
where
    T: Time,
    S: Default,
{
    pub fn new_with_time(time: T) -> Self {
        Self::new_with_time_and_state(time, Default::default())
    }
}

impl<T, S> Timewrap<T, S>
where
    T: Time + Default,
{
    pub fn new_with_state(state: S) -> Self {
        Self::new_with_time_and_state(Default::default(), state)
    }
}

impl<T, S> Timewrap<T, S>
where
    T: Time,
{
    pub fn new_with_time_and_state(time: T, state: S) -> Self {
        Self {
            data: Mutex::new(TimewrapData {
                current: time,
                tasks: HashMap::new(),
                waitings: BinaryHeap::new(),
            }),
            state,
            is_by_time: AtomicBool::new(false),
            current_id: AtomicUsize::new(0),
            #[cfg(feature = "drive_shared")]
            shared_lock: futures::lock::Mutex::new(()),
        }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn into_state(self) -> S {
        self.state
    }

    fn spawn_inner<'a>(
        &'a self,
        f: Pin<Box<dyn Future<Output = ()> + Send + 'a>>,
        time: Option<T>,
    ) {
        let id = as_ptr(f.as_ref().get_ref()) as *const () as usize;
        let mut this = self.data.lock();
        let this = &mut *this;
        this.tasks.insert(id, unsafe { mem::transmute(f) });
        this.waitings.push(Waiting {
            id,
            time: time.unwrap_or_else(|| this.current.clone()),
        })
    }

    pub fn spawn<F>(&self, f: F)
    where
        F: for<'a> FnOnce(
            TimewrapHandle<'a, T, S>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>,
    {
        self.spawn_inner(f(TimewrapHandle { target: self }), None);
    }

    pub fn spawn_at<F>(&self, time: T, f: F)
    where
        F: for<'a> FnOnce(
            TimewrapHandle<'a, T, S>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>,
    {
        self.spawn_inner(f(TimewrapHandle { target: self }), Some(time));
    }

    async fn inner_drive(&self, time: T) {
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

    pub async fn drive(&mut self, time: T) {
        self.inner_drive(time).await
    }

    #[cfg(feature = "drive_shared")]
    pub async fn drive_shared(&self, time: T) {
        let lock = self.shared_lock.lock().await;
        self.inner_drive(time).await;
        drop(lock);
    }

    #[cfg(feature = "drive_block")]
    pub fn drive_block(&mut self, time: T) {
        futures::executor::block_on(self.drive(time));
    }

    #[cfg(feature = "drive_shared")]
    #[cfg(feature = "drive_block")]
    pub fn drive_shared_block(&self, time: T) {
        futures::executor::block_on(self.drive_shared(time));
    }
}

impl<'a, T, S> TimewrapHandle<'a, T, S>
where
    T: Time,
{
    pub fn at(self, time: T) -> At<'a, T, S> {
        At {
            timewrap: self.target,
            time,
        }
    }

    pub fn spawn(self, f: impl Future<Output = ()> + Send + 'a) {
        self.target.spawn_inner(Box::pin(f), None);
    }

    pub fn spawn_at(self, f: impl Future<Output = ()> + Send + 'a, time: T) {
        self.target.spawn_inner(Box::pin(f), Some(time));
    }

    pub fn state(self) -> &'a S {
        &self.target.state
    }
}

impl<'a, T, S> TimewrapHandle<'a, T, S>
where
    T: Time + Add<Output = T>,
{
    pub fn delay(self, time: T) -> At<'a, T, S> {
        At {
            timewrap: self.target,
            time: time + self.target.data.lock().current.clone(),
        }
    }
}

#[must_use]
pub struct At<'a, T, S>
where
    T: Time,
{
    timewrap: &'a Timewrap<T, S>,
    time: T,
}

impl<'a, T, S> Future for At<'a, T, S>
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
