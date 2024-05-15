use alloc::{string::String, sync::Arc, vec::Vec};

use crate::{fs::dentry::path_to_dentry, process::{hart::cpu::init_cpu_locals, schedule::init_schedule}, utils::path::parent_path};

use self::{pcb::PCB, pid::init_pid_allocator, schedule::SCHEDULE};

pub mod context;
pub mod hart;
pub mod pcb;
pub mod pid;
pub mod switch;
pub mod trap;
pub mod schedule;
pub mod kstack;
pub mod loader;

// lazy_static! {
//     pub static ref ORIGIN_TASK: Arc<PCB> = Arc::new(
//         PCB::elf_data_to_pcb("file_name", &[0])
//     );
// }

pub static mut ORIGIN_TASK: Option<Arc<PCB>> = None;
// pub static ORIGIN_TASK: Option<Arc<SpinLock<PCB>>> = None;

pub fn init_origin_task() {
    // 1. 先拿到elf的data数据
    let path = "/brk";
    let dentry = path_to_dentry(path);
    if dentry.is_none() {
        panic!("No file:{} in file system.", path);
    }
    let inode = Arc::clone(&dentry.as_ref().unwrap().metadata().inner.lock().d_inode);
    let data = inode.read_all();
    // 2. 然后再根据这些数据构造pcb
    init_pid_allocator();
    init_cpu_locals();
    init_testcase();
    
    unsafe {
        // 5.10 修改了path这里传输的值
        ORIGIN_TASK = Some(Arc::new(PCB::elf_data_to_pcb(&parent_path(path), &data)));
    }
    // println!("Origin task initialization finished!");
    init_schedule();
}

pub fn init_task_and_push(elf_name: &str) {
    let mut path = String::from("/");
    path.push_str(elf_name);
    let dentry = path_to_dentry(&path);
    if dentry.is_none() {
        panic!("No file:{} in file system.", path);
    }
    let inode = Arc::clone(&dentry.as_ref().unwrap().metadata().inner.lock().d_inode);
    let data = inode.read_all();
    let pcb = Arc::new(PCB::elf_data_to_pcb(&parent_path(&path), &data));
    SCHEDULE.lock().task_queue.push_back(Arc::clone(&pcb));
}

// 这里有个别测例的顺序需要修改。
// 原因：部分测例会统一使用/mnt目录去进行挂载操作，但是在mount之后，会执行umount操作，会将/mnt将其从树上删掉。
// 而后续的测例依然认为存在/mnt目录，所以造成了一些莫名其妙的错误。
#[allow(unused)]
pub const PRELIMINARY_TESTCASES: &[&str] = &[
    "brk",
    "umount",
    "mount",
    "mmap",
    "openat",
    "chdir",
    "clone",
    "close",
    "dup",
    "dup2",
    "execve",
    "exit",
    "fork",
    "fstat",
    "getcwd",
    "getdents",
    "getpid",
    "getppid",
    "gettimeofday",
    "mkdir_",
    "munmap",
    "open",
    "pipe",
    "read",
    "sleep",
    "times",
    "uname",
    "unlink",
    "wait",
    "waitpid",
    "write",
    "yield",
];

pub static mut TESTCASE: Vec<&str> = Vec::new();

pub fn init_testcase() {
    for i in 1..PRELIMINARY_TESTCASES.len() {
        unsafe {
            TESTCASE.push(PRELIMINARY_TESTCASES[i]);
        }
    }
} 