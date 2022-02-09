use super::*;
use std::collections::LinkedList;

pub struct Mutex<T> {
    writers: HashMap<T, LinkedList<Waker>>,
}

impl<T: Eq + Hash + Copy + Unpin> Mutex<T> {
    #[inline]
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
        UntilUnlocked {
            num,
            writers: &self.writers,
            state: false,
            init: true,
        }
        .await;
        WriteGuard { num, inner: self }
    }

    fn unlock(&self, num: T) {
        let mut cell = self.writers.write(num);
        if let Some(waker) = unsafe { cell.get_mut().unwrap_unchecked() }.pop_front() {
            waker.wake();
        } else {
            cell.remove();
        }
    }

    #[inline]
    pub fn wait_for_unlock(&self, num: T) -> UntilUnlocked<T> {
        UntilUnlocked {
            num,
            state: false,
            writers: &self.writers,
            init: false,
        }
    }
}

pub struct UntilUnlocked<'a, T> {
    num: T,
    writers: &'a HashMap<T, LinkedList<Waker>>,
    state: bool,
    init: bool,
}

impl<T: Unpin + Hash + Eq + Copy> Future for UntilUnlocked<'_, T> {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.state {
            return Poll::Ready(());
        }
        let mut cell = self.writers.write(self.num);
        match cell.get_mut() {
            Some(wakers) => wakers.push_back(cx.waker().clone()),
            None => {
                if self.init {
                    cell.insert(LinkedList::new());
                }
                return Poll::Ready(());
            }
        }
        self.state = true;
        Poll::Pending
    }
}

pub struct WriteGuard<'a, T: Eq + Hash + Unpin + Copy> {
    num: T,
    inner: &'a Mutex<T>,
}

impl<T: Eq + Hash + Unpin + Copy> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.inner.unlock(self.num);
    }
}
