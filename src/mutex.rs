use std::collections::LinkedList;

use super::*;

struct Waiter {
    waker: Waker,
    state: *mut PollState,
}

unsafe impl Send for Waiter {}
unsafe impl Sync for Waiter {}

/// Super-fast asynchronous mutex implementation.
/// Implementation is based on the Binary [Semaphore](https://en.wikipedia.org/wiki/Semaphore_(programming)) algorithm.
#[derive(Default)]
pub struct Mutex<T> {
    map: Map<T, LinkedList<Waiter>>,
}

impl<T: Eq + Hash + Copy + Unpin> Mutex<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            map: new_map(),
        }
    }

    #[inline]
    pub fn is_locked(&self, num: &T) -> bool {
        self.map.lock().unwrap().contains_key(num)
    }

    pub fn unlock(&self, num: &T) {
        if let Some(list) = self.map.lock().unwrap().remove(num) {
            for waiter in list {
                // SAFETY: We have exclusive access to the `state`, so it is safe to mutate it.
                unsafe { *waiter.state = PollState::Ready };
                waiter.waker.wake();
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
    unsafe fn _wake_next(&self, num: &T) {
        let mut map = self.map.lock().unwrap_unchecked();
        let cell = map.get_mut(num);
        match cell.unwrap_unchecked().pop_front() {
            Some(waiter) => {
                *waiter.state = PollState::Ready;
                waiter.waker.wake();
            }
            None => {
                map.remove(num);
            }
        }
    }

    pub async fn lock(&self, num: T) -> WriteGuard<'_, T> {
        let mutex_guard = WriteGuard { num, inner: self };
        WaitForUnlock {
            num,
            map: &self.map,
            state: PollState::Init.into(),
        }
        .await;
        mutex_guard
    }
}

pub struct WriteGuard<'a, T: Eq + Hash + Copy + Unpin> {
    num: T,
    inner: &'a Mutex<T>,
}

// impl<T: ?Sized> !Send for MutexGuard<'_, T> {}
// impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<'a, T: Eq + Hash + Copy + Unpin> Drop for WriteGuard<'a, T> {
    fn drop(&mut self) {
        // SAFETY: LinkedList is properly initialized.
        unsafe { self.inner._wake_next(&self.num) };
    }
}

struct WaitForUnlock<'a, T> {
    num: T,
    map: &'a Map<T, LinkedList<Waiter>>,
    state: UnsafeCell<PollState>,
}

impl<'a, T: Eq + Hash + Copy + Unpin> Future for WaitForUnlock<'a, T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        ret_fut!(*this.state.get(), {
            let mut map = this.map.lock().unwrap();
            match map.get_mut(&this.num) {
                Some(w) => w.push_back(Waiter {
                    waker: cx.waker().clone(),
                    state: this.state.get(),
                }),
                None => {
                    map.insert(this.num, LinkedList::new());
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
                let mut map = this.inner.map.lock().unwrap();
                match map.get_mut(&this.num) {
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
                unsafe { this.inner._wake_next(&this.num) };
                return Poll::Ready(());
            }
            _ => {}
        }
        Poll::Pending
    }
}
