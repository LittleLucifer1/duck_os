//! 任务上下文, 用于任务切换

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TaskContext {
    // 在switch之后，应该返回的位置
    ra: usize,
    // 栈指针
    sp: usize,
    // 在switch的时候，需要保存的寄存器（caller saved）
    s: [usize; 12],
}

impl TaskContext {
    pub const fn empty() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    /* 普通的用户任务上下文切换
    1. 默认ra： __restore，即准备返回用户态
    2. 默认sp：用户内核栈指针，用于存储trap_cx信息，当返回内核态时，可以拿到相关的信息
    */ 
    pub fn init_task_cx(kernel_stack: usize) -> Self {
        extern "C" {
            fn __restore();
        }
        Self {
            ra: __restore as usize,
            sp: kernel_stack,
            s: [0; 12],
        }
    }
}