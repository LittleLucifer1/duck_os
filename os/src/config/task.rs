// The number of core
pub const MAX_CORE_NUM: usize = 8;

// The max number of pid
pub const MAX_PID: usize = 1024;

// 进程内核栈（用于存放trap）大小
pub const KERNEL_STACK_SIZE: usize = 1024 * 1024; // 1 MB

// 核的栈大小
pub const CORE_STACK_SIZE: usize = 16 * 4096;