//! 包装层 提供区间修改等服务

/*
    目前只有两种类型的需要有这个服务
        1）user_heap (sbrk)
        2) vma (mmap)
*/