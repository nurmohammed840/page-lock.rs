use super::*;

pub struct Mutex<T> {
     writers: DashMap<T, Vec<Waker>>,
}

impl<T: Eq + Hash + Copy + Unpin> Mutex<T> {
    pub fn new() -> Self {
        Self {
            writers: DashMap::new(),
        }
    }

    #[inline]
    pub fn is_locked(&self, num: T) -> bool {
        self.writers.contains_key(&num)
    }

    #[inline]
    pub fn wait_for_unlock(&self, num: T) -> UntilUnlocked<T> {
        UntilUnlocked {
            num,
            state: false,
            writers: &self.writers,
        }
    }

    pub fn unlock(&self, num: &T) {
        if let Some((_, wakers)) = self.writers.remove(num) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    pub async fn lock(&self, num: T) -> WriteGuard<'_, T> {
        self.wait_for_unlock(num).await;
        self.writers.insert(num, Vec::new());
        WriteGuard { num, inner: self }
    }
}

pub struct UntilUnlocked<'a, T> {
    num: T,
    state: bool,
    writers: &'a DashMap<T, Vec<Waker>>,
}

impl<T: Unpin + Hash + Eq> Future for UntilUnlocked<'_, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.state {
            return Poll::Ready(());
        }
        match this.writers.get_mut(&this.num) {
            Some(mut wakers) => wakers.push(cx.waker().clone()),
            None => return Poll::Ready(()),
        }
        this.state = true;
        Poll::Pending
    }
}

pub struct WriteGuard<'a, T: Eq + Hash + Unpin + Copy> {
    num: T,
    inner: &'a Mutex<T>,
}

impl<T: Eq + Hash + Unpin + Copy> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.inner.unlock(&self.num)
    }
}
