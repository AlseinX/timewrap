use core::{
    future::Future,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    pin::Pin,
    task::Poll,
};

pub struct PauseWhen<F, C>(pub F, pub C)
where
    F: Future,
    C: Fn() -> bool;

impl<F, C> Future for PauseWhen<F, C>
where
    F: Future,
    C: Fn() -> bool,
{
    type Output = Poll<F::Output>;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let Self(fut, when) = unsafe { self.get_unchecked_mut() };
        match unsafe { Pin::new_unchecked(fut) }.poll(cx) {
            Poll::Ready(r) => Poll::Ready(Poll::Ready(r)),
            Poll::Pending => {
                if when() {
                    Poll::Ready(Poll::Pending)
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

pub struct Erase<T, const SIZE: usize> {
    data: [u8; SIZE],
    _phantom: PhantomData<dyn AsRef<T>>,
}

unsafe impl<T, const SIZE: usize> Send for Erase<T, SIZE> {}
unsafe impl<T, const SIZE: usize> Sync for Erase<T, SIZE> {}

impl<T, const SIZE: usize> Erase<T, SIZE> {
    pub fn new(v: T) -> Self {
        debug_assert_eq!(mem::size_of::<T>(), SIZE);
        let s = Self {
            data: unsafe { (&v as *const T as *const [u8; SIZE]).read() },
            _phantom: PhantomData,
        };
        mem::forget(v);
        s
    }
}

impl<T, const SIZE: usize> Deref for Erase<T, SIZE> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { *(&&self.data as *const &[u8; SIZE] as *const &T) }
    }
}

impl<T, const SIZE: usize> DerefMut for Erase<T, SIZE> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { *(&mut &mut self.data as *mut &mut [u8; SIZE] as *mut &mut T) }
    }
}

impl<T, const SIZE: usize> Drop for Erase<T, SIZE> {
    fn drop(&mut self) {
        unsafe { ((&mut **self) as *mut T).drop_in_place() }
    }
}
