//! Super-fast asynchronous mutex implementation.
//! Implementation is based on the Binary [Semaphore](https://en.wikipedia.org/wiki/Semaphore_(programming)) algorithm.

use super::*;
use std::collections::LinkedList;

struct Waiter {
    waker: Waker,
    state: *mut PollState,
}

unsafe impl Send for Waiter {}
unsafe impl Sync for Waiter {}

#[derive(Default)]
pub struct Mutex<T> {
    map: HashMap<T, LinkedList<Waiter>>,
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Sync> Sync for Mutex<T> {}

impl<T: Eq + Hash + Copy + Unpin> Mutex<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    #[inline]
    pub fn is_locked(&self, num: &T) -> bool {
        self.map.read(num).contains_key()
    }

    pub fn unlock(&self, num: T) {
        if let Some(list) = self.map.write(num).remove() {
            for waiter in list {
                // SAFETY: We have exclusive access to the `state`, so it is safe to mutate it.
                unsafe { *waiter.state = PollState::Ready };
                waiter.waker.wake();
                // unsafe { *state = PollState::Ready };
            }
        }
    }

    #[inline]
    pub fn until_unlocked(&self, num: T) -> UntilUnlocked<T> {
        UntilUnlocked {
            num,
            inner: self,
            state: PollState::Init,
        }
    }

    /// SAFETY: Make sure that, `LinkedList<(...)>` is properly initialized in `HashMap`.
    unsafe fn _wake_next(&self, num: T) {
        let mut cell = self.map.write(num);
        match cell.get_mut().unwrap_unchecked().pop_front() {
            Some(waiter) => {
                *waiter.state = PollState::Ready;
                waiter.waker.wake();
            }
            None => {
                cell.remove();
            }
        }
    }

    pub async fn lock(&self, num: T) -> MutexGuard<'_, T> {
        let mutex_guard = MutexGuard { num, inner: self };
        WaitForUnlock {
            num,
            map: &self.map,
            state: PollState::Init,
        }
        .await;
        mutex_guard
    }
}

pub struct MutexGuard<'a, T: Eq + Hash + Copy + Unpin> {
    num: T,
    inner: &'a Mutex<T>,
}

// impl<T: ?Sized> !Send for MutexGuard<'_, T> {}
// impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<'a, T: Eq + Hash + Copy + Unpin> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        // SAFETY: LinkedList is properly initialized.
        unsafe { self.inner._wake_next(self.num) };
    }
}

struct WaitForUnlock<'a, T> {
    num: T,
    map: &'a HashMap<T, LinkedList<Waiter>>,
    state: PollState,
}

impl<'a, T: Eq + Hash + Copy + Unpin> Future for WaitForUnlock<'a, T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        ret_fut!(this.state, {
            let mut cell = this.map.write(this.num);
            match cell.get_mut() {
                Some(w) => w.push_back(Waiter {
                    waker: cx.waker().clone(),
                    state: &mut this.state,
                }),
                None => {
                    cell.insert(LinkedList::new());
                    return Poll::Ready(());
                }
            }
        });
    }
}

pub struct UntilUnlocked<'a, T> {
    num: T,
    inner: &'a Mutex<T>,
    state: PollState,
}

impl<'a, T: Eq + Hash + Copy + Unpin> Future for UntilUnlocked<'a, T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this.state {
            PollState::Init => {
                let mut cell = this.inner.map.write(this.num);
                match cell.get_mut() {
                    Some(w) => w.push_back(Waiter {
                        waker: cx.waker().clone(),
                        state: &mut this.state,
                    }),
                    None => return Poll::Ready(()),
                }
                this.state = PollState::Pending;
            }
            PollState::Ready => {
                // SAFETY: LinkedList is properly initialized.
                unsafe { this.inner._wake_next(this.num) };
                return Poll::Ready(());
            }
            _ => {}
        }
        Poll::Pending
    }
}
