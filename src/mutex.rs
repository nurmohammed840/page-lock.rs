use super::*;
use std::collections::LinkedList;

pub struct Mutex<T> {
    writers: HashMap<T, LinkedList<Waker>>,
}

impl<T: Eq + Hash + Copy + Unpin> Mutex<T> {
    pub fn new() -> Self {
        Self {
            writers: HashMap::new(),
        }
    }

    #[inline]
    pub fn is_locked(&self, num: &T) -> bool {
        self.writers.read(num).contains_key()
    }

    pub async fn lock(&self, num: T) -> WriteGuard<'_, T> {
        let mut pull_state = PollState::Init;
        UntilUnlocked {
            num,
            writers: &self.writers,
            state: &mut pull_state,
        }
        .await;

        WriteGuard {
            num,
            inner: self,
            state: pull_state,
        }
    }
}

pub struct UntilUnlocked<'a, T> {
    num: T,
    writers: &'a HashMap<T, LinkedList<Waker>>,
    state: &'a mut PollState,
}

impl<T: Unpin + Hash + Eq + Clone> Future for UntilUnlocked<'_, T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        *this.state = poll_state_ready!(this.state);

        let mut cell = this.writers.write(this.num.clone());
        match cell.get_mut() {
            Some(wakers) => {
                wakers.push_back(cx.waker().clone());
                Poll::Pending
            }
            None => {
                cell.insert(LinkedList::new());
                Poll::Ready(())
            }
        }
    }
}

pub struct WriteGuard<'a, T: Eq + Hash + Unpin + Copy> {
    num: T,
    inner: &'a Mutex<T>,
    state: PollState,
}

impl<T: Eq + Hash + Unpin + Copy> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.state = PollState::Ready;
        let mut cell = self.inner.writers.write(self.num);
        if let Some(waker) = unsafe { cell.get_mut().unwrap_unchecked() }.pop_front() {
            waker.wake();
        } else {
            cell.remove();
        }
    }
}
