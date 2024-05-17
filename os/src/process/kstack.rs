//! 用户进程的内核栈，存储trap的地方

use alloc::vec::Vec;

use crate::{config::{mm::PAGE_SIZE, task::KERNEL_STACK_SIZE}, mm::allocator::frame::{alloc_contiguous_frame, FrameTracker}};

use super::trap::context::TrapContext;


// TODO：暂时先这样，主要是还没有弄明白frame和page的区别！
pub struct Kstack {
    pub frames: Vec<FrameTracker>,
}

impl Kstack {
    pub fn init_kernel_stack() -> Self {
        let size = KERNEL_STACK_SIZE / PAGE_SIZE;
        if let Some(frames) = alloc_contiguous_frame(size) {
            Kstack {frames}
        } else {
            panic!()
        }
    }
    // 高地址，这样的话，当插入数据时，需要向下腾出空间，数据放入即可，且数据头在低地址上。
    fn kstack_top_ptr(&self) -> usize {
        self.frames.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    // 返回插入后的栈顶
    pub fn push_trap_cx(&self, trap_cx: TrapContext) -> usize {
        let ptr = (self.kstack_top_ptr() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *ptr = trap_cx;
        }
        ptr as usize
    }

    pub fn top_trap_cx(&self) -> &TrapContext {
        let top_addr = self.kstack_top_ptr() - core::mem::size_of::<TrapContext>();
        unsafe { &*(top_addr as *const TrapContext)}
    }
}

