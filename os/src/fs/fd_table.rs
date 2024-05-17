//! 文件描述符表

use alloc::{ sync::Arc, vec::Vec};
use bitmap_allocator::{BitAlloc, BitAlloc256};
use hashbrown::HashMap;

use crate::{config::fs::MAX_FD, syscall::error::OSResult};

use super::{dentry::Dentry, file::File, info::OpenFlags, stdio::{Stderr, Stdin, Stdout, STDERR, STDIN, STDOUT}};

pub struct FdAllocator {
    pub bitmap: BitAlloc256,
}

impl FdAllocator {
    pub fn new() -> Self {
        let mut allocator = Self {
            bitmap: BitAlloc256::DEFAULT,
        };
        allocator.bitmap.insert(3..MAX_FD);
        allocator
    }

    pub fn alloc_fd(&mut self) -> Option<usize> {
        self.bitmap.alloc()
    }

    pub fn alloc_spec_fd(&mut self, new_fd: usize) -> bool {
        // 如果没有被分配
        if self.bitmap.test(new_fd) {
            self.bitmap.remove(new_fd..new_fd+1);
            true
        } else {
            false
        }
    }

    pub fn dealloc(&mut self, fd: usize) {
        self.bitmap.dealloc(fd);
    }
}

// 这里的fd没有实现RAII，之后根据需求再判断要不要实现
// TODO: 这里的fd_table完全可以更换为 hash table，而不采用BTreeMap
pub struct FdTable {
    pub fd_table: HashMap<usize, FdInfo>,
    pub fd_allocator: FdAllocator,
}

impl FdTable {
    // 插入file，返回出这个file的文件描述符
    pub fn insert_get_fd(&mut self, fd_info: FdInfo) -> usize {
        let fd = self.fd_allocator.alloc_fd().unwrap();
        self.fd_table.insert(fd, fd_info);
        fd
    }

    // 插入到特定的fd中，返回插入是否成功。不成功意味着这个new_fd已经在使用了
    pub fn insert_spec_fd(&mut self, new_fd: usize, fd_info: FdInfo) -> OSResult<bool> {
        if self.fd_allocator.alloc_spec_fd(new_fd) {
            self.fd_table.insert(new_fd, fd_info);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn init_fdtable() -> Self {
        let mut fd_table = HashMap::new();
        fd_table.insert(
            STDIN, 
            FdInfo::new(Arc::new(Stdin), OpenFlags::O_RDONLY)
        );
        fd_table.insert(
            STDOUT, 
            FdInfo::new(Arc::new(Stdout), OpenFlags::O_WRONLY)
        );
        fd_table.insert(
            STDERR, 
            FdInfo::new(Arc::new(Stderr), OpenFlags::O_WRONLY)
        );
        let fd_allocator = FdAllocator::new();
        Self { fd_table, fd_allocator }
    }

    pub fn close_exec(&mut self) {
        let mut remove_fd: Vec<usize> = Vec::new();
        for fd in self.fd_table.keys() {
            if self.fd_table.get(fd)
                .unwrap()
                .flags.contains(OpenFlags::O_CLOEXEC) {
                remove_fd.push(*fd);
            }
        }
        for fd in remove_fd {
            self.close(fd);
        }
    }

    pub fn from_clone_copy(&self) -> Self {
        let fd_table = self.fd_table.clone();
        let mut new_fd_table = FdTable::init_fdtable();
        for &fd in fd_table.keys() {
            if fd < 3 {
                continue;
            }
            let ret = new_fd_table.fd_allocator.alloc_spec_fd(fd);
            assert!(ret == true);
            let file = fd_table.get(&fd).unwrap().clone();
            new_fd_table.fd_table.insert(fd, file);
        }
        new_fd_table
    }

    pub fn open(&mut self, dentry: Arc<dyn Dentry>, flags: OpenFlags) -> OSResult<usize> {
        let file = dentry.open(Arc::clone(&dentry), flags.clone())?;
        let fd_info = FdInfo::new(file, flags);
        Ok(self.insert_get_fd(fd_info))
    }

    pub fn close(&mut self, fd: usize) {
        self.fd_table.remove(&fd);
        self.fd_allocator.dealloc(fd);
    }
}

#[derive(Clone)]
pub struct FdInfo {
    pub file: Arc<dyn File>,
    pub flags: OpenFlags,
}

impl FdInfo {
    pub fn new(file: Arc<dyn File>, flags: OpenFlags) -> Self {
        Self { file, flags }
    }
}


#[allow(unused)]
pub fn test_fdallocator() {
    let mut allocator = FdAllocator::new();
    let mut fds = Vec::new();
    for i in 0..10 {
        fds.push(allocator.alloc_fd().unwrap());
    }
    println!("before {:?}", fds);
    for i in 0..8 {
        if let Some(id) = fds.pop() {
            allocator.dealloc(id);
        }
    }
    println!("after {:?}", fds);

    while !fds.is_empty() {
        let a = fds.pop();
        allocator.dealloc(a.unwrap());
    }
    
    let a1 = allocator.alloc_fd();
    let a2 = allocator.alloc_fd();
    println!("Allocate a1: {}, and a2: {}", a1.unwrap(), a2.unwrap());
    allocator.dealloc(a1.unwrap());
    let a3 = allocator.alloc_fd();
    println!("The a3 is {}", a3.unwrap());

    for i in 0..2 {
        let ret = allocator.alloc_spec_fd(100);
        println!("The {} is {}", i, ret);
    }
}