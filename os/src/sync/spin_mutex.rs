//! spin_mutex模块

use core::{cell::UnsafeCell, marker::PhantomData, ops::{Deref, DerefMut}, sync::atomic::{AtomicBool, Ordering}};

use super::MutexAction;

pub struct SpinMutex<T:?Sized, Action: MutexAction> {
    _marker: PhantomData<Action>,
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

pub struct SpinMutexGuard<'a, T:?Sized + 'a, Action: MutexAction> {
    mutex: &'a SpinMutex<T, Action>,
    _marker: PhantomData<Action>,
}

unsafe impl<T: ?Sized + Send, Action: MutexAction> Sync for SpinMutex<T, Action> {}
unsafe impl<T: ?Sized + Send, Action: MutexAction> Send for SpinMutex<T, Action> {}
unsafe impl<T: ?Sized + Sync, Action: MutexAction> Sync for SpinMutexGuard<'_, T, Action> {}
unsafe impl<T: ?Sized + Send, Action: MutexAction> Send for SpinMutexGuard<'_, T, Action> {}

impl<T, Action: MutexAction> SpinMutex<T, Action> {
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        SpinMutex {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
            _marker: PhantomData,
        }
    }
}

impl<T: ?Sized, Action: MutexAction> SpinMutex<T, Action> {
    #[inline(always)]
    pub fn lock(&self) -> SpinMutexGuard<'_, T, Action> {
        Action::before_lock();
        loop {
            let mut count: usize = 0;
            while self.lock.load(Ordering::Relaxed) {
                core::hint::spin_loop();
                count += 1;
                if count == 0x1000_0000 {
                    println!("Dead Lock!");
                    todo!()
                }
            }
            if self
                .lock
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok() {
                    break
                }
            // else {
            //     // case：此时已经是true，但是还是拿不到锁，所以应该要报错
            //     todo!()
            // }
        }
        SpinMutexGuard {
            mutex: &self,
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    pub fn try_lock(&self) -> Option<SpinMutexGuard<'_, T, Action>> {
        Action::before_lock();
        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok() {
                Some(SpinMutexGuard {
                    mutex: &self,
                    _marker: PhantomData,
                })
            }
        else {
            Action::after_lock();
            None
        }
    }
}

impl <'a, T:?Sized, Action: MutexAction> Deref for SpinMutexGuard<'a, T, Action> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl <'a, T:?Sized, Action: MutexAction> DerefMut for SpinMutexGuard<'a, T, Action> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl <'a, T: ?Sized, Action: MutexAction> Drop for SpinMutexGuard<'a, T, Action> {
    fn drop(&mut self) {
        self.mutex.lock.store(false, Ordering::Release);
        Action::after_lock()
    }
}