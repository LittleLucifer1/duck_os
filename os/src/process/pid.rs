use bitmap_allocator::{BitAlloc, BitAlloc4K};

use crate::{config::task::MAX_PID, sync::SpinLock};

type PidAllocatorImpl = BitAlloc4K;

trait PidAllocator {
    fn init(&mut self);
    fn alloc_pid(&mut self) -> Option<Pid>;
    fn dealloc_pid(&mut self, pid: usize);
}

impl PidAllocator for PidAllocatorImpl {
    // The root pid = 1
    fn init(&mut self) {
        self.insert(2..MAX_PID);
    }
    
    fn dealloc_pid(&mut self, pid: usize) {
        self.dealloc(pid)
    }

    fn alloc_pid(&mut self) -> Option<Pid> {
        Some(Pid::init(self.alloc()?))
    }
}

pub static PID_ALLOCATOR: SpinLock<PidAllocatorImpl> = SpinLock::new(PidAllocatorImpl::DEFAULT);

pub struct Pid {
    pub value: usize,
}

impl Pid {
    pub fn init(value: usize) -> Self {
        Self { value }
    }
}

impl Drop for Pid {
    fn drop(&mut self) {
        dealloc_pid(self.value)
    }
}

pub fn init_pid_allocator() {
    PID_ALLOCATOR.lock().init();
}

pub fn alloc_pid() -> Option<Pid> {
    PID_ALLOCATOR.lock().alloc_pid()
}

pub fn dealloc_pid(pid: usize) {
    PID_ALLOCATOR.lock().dealloc(pid)
}