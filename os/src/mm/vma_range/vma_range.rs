use alloc::{collections::BTreeMap, vec::Vec};

use crate::{
    config::mm::{MMAP_BOTTOM, MMAP_TOP}, 
    mm::{page_table::PageTable, type_cast::MapPermission, vma::VirtMemoryAddr}, syscall::error::{Errno, OSResult}
};

use super::{SplitOverlap, UnmapOverlap};

// #[derive(Debug)]
pub struct VmaRange {
    pub segments: BTreeMap<usize, VirtMemoryAddr>,
}

impl VmaRange {
    // 初始化
    pub fn new() -> VmaRange {
        VmaRange {
            segments: BTreeMap::new(),
        }
    }
    // 插入一段虚拟逻辑段，不检查 用于mmap的任意一个内存逻辑段
    pub fn insert_raw(&mut self, vma: VirtMemoryAddr) {
        self.segments.insert(vma.start_vaddr, vma);
    }

    // mmap_fixed
    pub fn find_fixed(
        &mut self, 
        start: usize, 
        end: usize,
        pt: &mut PageTable,
    ) -> Option<usize> {
        // TODO: 检查这个地址 同时这里仅仅unmap，没有这么简单！
        self.unmap(start, end, pt);
        Some(start)
    }
    
    // 查找空的空间
    // 如果hint为0,则从最下面LOW_LIMIT开始分配空间
    // 如果hint不为0,则之前会保证其会在一个相对合理的位置，最后实在不行就分配在最高的那个vma的上面。
    // TODO：如果没有找到，应该要报错。之后把mmap的区域加大些
    pub fn find_anywhere(&self, hint: usize, len: usize) -> Option<usize> {
        let mut last_end = hint.max(MMAP_BOTTOM);
        for (&start, vma) in self.segments.iter() {
            if last_end + len <= start {
                return Some(last_end);
            }
            last_end = last_end.max(vma.end_vaddr);
        }
        if last_end + len <= MMAP_TOP {
            Some(last_end)
        } else {
            None
        }
    }

    // unmap一段区间，要检查 
    // 适用在mmap_fixed时，需要删除掉要等待分配区间的虚拟地址
    // 用在 munmap()函数中
    pub fn unmap(&mut self, start: usize, end: usize, pt: &mut PageTable) {
        if start == end {
            return; 
        }
        let mut idx_vec:Vec<usize> = Vec::new();
        for (idx, vma) in self.segments.iter() {
            if vma.is_overlap(start, end) == true {
                idx_vec.push(*idx);
            }
        }
        for i in idx_vec {
            if let Some(vma) = self.segments.get_mut(&i) {
                match vma.unmap_if_overlap(start, end, pt) {
                    UnmapOverlap::Split(right_vma) => {
                        self.segments.insert(right_vma.start_vaddr, right_vma);
                    }
                    _ => {}
                }
            }
        }
        
    }

    // 用在mmap_protect函数中
    pub fn mprotect(&mut self, start: usize, end: usize, new_flags: MapPermission, pt: &mut PageTable) {
        let mut idx_vec: Vec<usize> = Vec::new();
        for (idx, vma) in self.segments.iter() {
            if vma.is_overlap(start, end) == true {
                idx_vec.push(*idx);
            }
        }
        for i in idx_vec {        
            if let Some(vma) = self.segments.get_mut(&i) {
                match vma.split_and_modify_if_overlap(start, end, new_flags, pt) {
                    SplitOverlap::ShrinkLeft(right_vma) => {
                        self.segments.insert(right_vma.start_vaddr, right_vma);
                    }
                    SplitOverlap::ShrinkRight(right_vma) => {
                        self.segments.insert(right_vma.start_vaddr, right_vma);
                    }
                    SplitOverlap::Split(middle_vma, right_vma) => {
                        self.segments.insert(right_vma.start_vaddr, right_vma);
                        self.segments.insert(middle_vma.start_vaddr, middle_vma);
                    }
                    _ => {}
                }
            }
        }   
    }

    // 用来扩展地址空间，目前只是用在brk系统调用中, 同时这里expand不会分配相关的页面
    pub fn expand(&mut self, start: usize, end: usize) -> OSResult<bool> {
        // 1. 检查该地址会不会和其他的地址重合
        let mut heap_vm: Option<&mut VirtMemoryAddr> = None;
        for (_, vma) in self.segments.iter_mut() {
            if vma.end_vaddr == start {
                heap_vm = Some(vma);
            } else if vma.is_overlap(start, end) {
                return Err(Errno::ENOMEM);
            }
        }
        if heap_vm.is_none() {
            return Err(Errno::ENOMEM);
        }
        heap_vm.unwrap().expand(end);
        Ok(true)
    }
}