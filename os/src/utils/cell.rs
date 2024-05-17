//! 这个模块是得到内部可见性的
//! 使用起来还是有些疑惑。

// SyncUnsafeCell: 内部可见性 + Sync； 编译器无法保证或者使用者自己确保 借用规则
// RefCell: 内部可见性 + 确保借用规则； 没有实现Sync
// 目前的问题：我想要得到 内部可见性 + 确保借用规则 + Sync， 但是我在代码中没有找到 Sync的使用情况且SyncUnsafeCell没有那么好
// 1. 如果代码中没有Sync需要用到的地方，则我可以直接使用 RefCell。
// 2. 如果我自己可以想清楚，确保借用规则。则可以使用 SyncUnsafeCell.
// 3. 如果我什么都想要，那么或许可以自己完成 RwLock<T>, 这个只在std库中。
// Reference: https://rustwiki.org/zh-CN/core/cell/index.html
// 又去看了一堆的博客，发现这个问题没有那么简单。本质上应该是去 仔细看相关数据结构的使用情况（会不会出现冲突）研究需求，然后再根据文档去找方案。

/// 
pub struct SyncUnsafeCell<T>(core::cell::SyncUnsafeCell<T>);

impl <T> SyncUnsafeCell<T> {
    ///
    #[inline]
    pub fn new(value: T) -> Self {
        Self(core::cell::SyncUnsafeCell::new(value))
    }

    /// TODO: 什么时候使用unchecked_mut 或者是 get_mut 仍然是一个问题
    /// TODO: 现在先不管它，全部使用 get_unchecked_mut 函数
    pub fn get_unchecked_mut(&self) -> &mut T {
        unsafe {
            &mut *self.0.get()
        }
    }

    ///
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    ///
    #[inline]
    pub fn get(&self) -> *mut T {
        self.0.get()
    }
}