//! 包装层 提供区间修改等服务

/*
    目前只有两种类型的需要有这个服务
        1）user_heap (sbrk)
        2) vma (mmap)
*/

use super::vma::VirtMemoryAddr;

pub mod vma_range;

pub enum UnmapOverlap {
    Unchange,
    Shrink,
    Removed,
    Split(VirtMemoryAddr),
}

pub enum SplitOverlap {
    Unchange,
    // 原来的区间只保留左边，分裂出修改过的右边
    ShrinkRight(VirtMemoryAddr),
    // 原有的区间保留右边，左边修改。但是返回的是未修改的右边区间
    ShrinkLeft(VirtMemoryAddr),
    // 原有的被全部修改
    Modified,
    // 原有的区间只有中间被修改，分裂中间的和右边的
    Split(VirtMemoryAddr, VirtMemoryAddr),

}