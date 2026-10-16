//! Single-threaded executor on a virtual millisecond clock.
//!
//! The original engine is written as nested blocking loops paced by `timeGetTime()`. The port
//! keeps that structure as `async` code: every place the original busy-waits becomes
//! `clock.until(t).await`, and the frontend drives the clock forward in real time.

use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

#[derive(Clone, Default)]
pub struct Clock(Rc<ClockState>);

#[derive(Default)]
struct ClockState {
    now: Cell<u64>,
    wake_at: Cell<u64>,
}

impl Clock {
    pub fn now(&self) -> u64 {
        self.0.now.get()
    }

    /// Resolves once the virtual clock reaches `t`.
    pub fn until(&self, t: u64) -> Until {
        Until { clock: self.clone(), t }
    }

    /// Yields to the frontend once without advancing time (used after presenting a frame).
    pub fn yield_now(&self) -> Until {
        self.until(self.now() + 1)
    }
}

pub struct Until {
    clock: Clock,
    t: u64,
}

impl Future for Until {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        if self.clock.now() >= self.t {
            Poll::Ready(())
        } else {
            self.clock.0.wake_at.set(self.t);
            Poll::Pending
        }
    }
}

pub struct Executor<T> {
    clock: Clock,
    fut: Option<Pin<Box<dyn Future<Output = T>>>>,
    result: Option<T>,
}

impl<T> Executor<T> {
    pub fn new(clock: Clock, fut: impl Future<Output = T> + 'static) -> Self {
        Self { clock, fut: Some(Box::pin(fut)), result: None }
    }

    /// Advance the virtual clock to `t`, running the task at every wake-up on the way so it sees
    /// exactly the times it asked for. `on_wake` runs before each poll (to feed input).
    pub fn run_until(&mut self, t: u64, mut on_wake: impl FnMut(u64)) {
        let mut cx = Context::from_waker(Waker::noop());
        while let Some(fut) = self.fut.as_mut() {
            let wake = self.clock.0.wake_at.get();
            if wake > t {
                break;
            }
            self.clock.0.now.set(wake.max(self.clock.now()));
            on_wake(self.clock.now());
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                self.result = Some(v);
                self.fut = None;
            }
        }
        if self.clock.now() < t {
            self.clock.0.now.set(t);
        }
    }

    pub fn finished(&self) -> Option<&T> {
        self.result.as_ref()
    }

    pub fn next_wake(&self) -> u64 {
        self.clock.0.wake_at.get()
    }
}
