//! dentry模块
//! 

/*
    所有的dentry构成一棵树的形式。并且应该存放在cache中。
    其实没有什么功能，就是方便查找路径。
    1. 数据结构
        1）name: 目录项名称（短名）
        2）inode：相关联的 inode
        3）path：路径名 
        4）parent: 父目录项
        5）children：子目录项
    
    2. 功能
        1） 生成 hash值，方便放入hash table中（待定）
*/


use core::any::Any;

use alloc::{collections::BTreeMap, string::{String, ToString}, sync::{Arc, Weak}, vec::Vec};
use hashbrown::HashMap;
use lazy_static::lazy_static;

use crate::{sync::SpinLock, syscall::error::OSResult, utils::path::format_path};

use super::{file::File, file_system::FILE_SYSTEM_MANAGER, info::{InodeMode, OpenFlags}, inode::Inode};

// TODO: 很多细节：诸如 函数正确性和逻辑、函数假设是否满足、path是否规范、Option、锁之类的都没考虑完全。

pub struct DentryMeta {
    pub inner: SpinLock<DentryMetaInner>,
}

// 这些数据都是可能会被修改的，所以用锁保护起来。
pub struct DentryMetaInner {
    pub d_name: String,
    pub d_path: String,
    pub d_inode: Arc<dyn Inode>,
    pub d_parent: Option<Weak<dyn Dentry>>,
    // (name, dentry)
    pub d_child: BTreeMap<String, Arc<dyn Dentry>>,
}

impl DentryMeta {
    pub fn new(
        name: String,
        path: String,
        inode: Arc<dyn Inode>,
        parent: Option<Arc<dyn Dentry>>,
        child: BTreeMap<String, Arc<dyn Dentry>>,
    ) -> Self {
        let name = format_path(&name);
        let parent = match parent {
            Some(parent) => Some(Arc::downgrade(&parent)),
            None => None,
        };
        Self { 
            inner: SpinLock::new(
                DentryMetaInner {
                    d_name: name,
                    d_path: path,
                    d_inode: inode,
                    d_parent: parent,
                    d_child: child
                }
            )
        }
    }
}

pub trait Dentry: Sync + Send + Any {
    fn metadata(&self) -> &DentryMeta;
    fn load_child(&self, this: Arc<dyn Dentry>);
    fn load_all_child(&self, this: Arc<dyn Dentry>);
    // 查找inode的子节点，并返回对应的dentry,负责把child_inode创建好，挂在dentry上
    // 从磁盘上找相关的数据，然后建立对应的dentry,并返回
    fn open(&self, dentry: Arc<dyn Dentry>, flags: OpenFlags) -> OSResult<Arc<dyn File>>;
    fn create(&self, this: Arc<dyn Dentry>, name: &str, mode: InodeMode) -> OSResult<Arc<dyn Dentry>>;
    fn mkdir(&self, path: &str, mode: InodeMode) -> Arc<dyn Dentry>;
    fn mknod(&self, path: &str, mode: InodeMode, dev_id: Option<usize>) -> Arc<dyn Dentry>;
    fn unlink(&self, child: Arc<dyn Dentry>);
    fn list_child(&self) {
        let meta_inner = self.metadata().inner.lock();
        println!("cwd: {} --", meta_inner.d_path);
        for (name, _child) in meta_inner.d_child.iter() {
            println!("       |-> {}", name);
        }
    }
}

lazy_static! {
    // (path, Dentry)
    // TODO: 或许需要换成radix tree？因为需要有前缀查找的功能
    pub static ref DENTRY_CACHE: SpinLock<HashMap<String, Arc<dyn Dentry>>> = SpinLock::new(HashMap::new());
}

// Assumption: 此时的path是合法的绝对路径，并且是format的
// function: 在Cache中查找dentry
pub fn path_to_dentry_cache(path: &str) -> Option<Arc<dyn Dentry>> {
    if let Some(dentry) = DENTRY_CACHE
        .lock()
        .get(path) {
            Some(Arc::clone(dentry))
        } else {
            None
        }
}
// Assumption: 此时的path是合法的绝对路径，并且是format的
// return：路径对应的dentry 相关的dentry已经在树上和cache中了
// TODO: 这里的meta反复的上锁，会不会出现问题？还是直接得到一个上了锁的，再统一修改。
/*
    1. openat：这个函数用来查找父dentry，此时的父dentry一定在cache中或者在树上。所以一定可以找到。
*/
pub fn path_to_dentry(path: &str) -> Option<Arc<dyn Dentry>> {
    // 绝对路径在cache中查找
    if let Some(dentry) = path_to_dentry_cache(path){
        Some(Arc::clone(&dentry))
    } else {
        // 没找到，匹配前最大子串
        // TODO：优化部分，这里可以匹配一下cache中的最大匹配前字串，从而减少查找的时间
        // 1. 如果找到了前最大子串，则pa_dentry不是从根开始

        // 2. 否则从根开始
        let mut pa_dentry = FILE_SYSTEM_MANAGER.root_dentry();
        let path_vec: Vec<&str> = path
            .split('/')
            .filter(|name| *name != "" )
            .collect();
        // 开始遍历所有的路径
        for name in path_vec.into_iter() {
            // 先从树上找
            if let Some((_, dentry)) = pa_dentry
                .clone()
                .metadata()
                .inner
                .lock()
                .d_child
                .iter()
                .find(|(n , _)| name.eq(*n) ) {
                    // 在树上找到了，插入cache中，然后继续遍历
                    DENTRY_CACHE.lock().insert(dentry.metadata().inner.lock().d_path.to_string(), dentry.clone());
                    pa_dentry = dentry.clone();
                }
            else {
                // 树上没有找到，说明这个文件不存在，如果是在open函数中，则可能要创建，其他的就直接报错！
                // 因为我确保了磁盘上每一个dentry都在树上 ----> 1. 初始化时所有的都在树上 2.创建新的dentry也给挂在树上
                return None;
            }
        }
        Some(pa_dentry)
    }
}