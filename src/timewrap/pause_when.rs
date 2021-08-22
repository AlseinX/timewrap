use core::{future::Future, pin::Pin, task::Poll};

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
