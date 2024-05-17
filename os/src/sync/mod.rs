use self::{interrupt::{push_off, push_on}, spin_mutex::SpinMutex};

pub mod spin_mutex;
pub mod interrupt;

pub type SpinLock<T> = SpinMutex<T, SpinIrq>;
pub type SpinNoIrqLock<T> = SpinMutex<T, SpinNoIrq>;

pub trait MutexAction {
    fn before_lock();
    fn after_lock();
}

// 自旋锁，有内核中断
pub struct SpinIrq;

impl MutexAction for SpinIrq {
    fn before_lock() {}

    fn after_lock() {}
}

// 自旋锁，无内核中断
pub struct SpinNoIrq;

impl MutexAction for SpinNoIrq {
    fn before_lock() {
        push_off()
    }

    fn after_lock() {
        push_on()
    }
}