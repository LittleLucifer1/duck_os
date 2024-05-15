use alloc::{string::String, vec::Vec};
use log::info;

use crate::{
    fs::{dentry::path_to_dentry, info::InodeMode, AT_FDCWD}, 
    process::hart::{cpu::{exit_current_task, get_cpu_id, get_cpu_local, suspend_current_task}, env::SumGuard}, 
    utils::{flag_check::check_clone_flags, path::ptr_and_dirfd_to_path, string::c_ptr_to_string}
};

use super::{error::{Errno, SyscallResult}, CloneFlags, Wait4Option};

// clone wait exevc yield (fork 使用的是clone)

/* Description: 立即终止进程
    注意事项： 1. 关闭所有打开的 fd
        2. 儿子进程要转移到init中
        3. 向parent发送一个SIGCHILD信号
    TODO：由于父进程仍然拥有当前的Arc进程，或许需要自己手动提前释放内存等相关资源。
*/
pub fn sys_exit(exit_code: i32) -> ! {
    info!("[sys_exit]: exit_code is {}", exit_code);
    exit_current_task(exit_code);
    unreachable!();
}

/* Description: 放弃当前的进程
    Return: 总是成功
 */
pub fn sys_yield() -> SyscallResult {
    info!("[sys_yield]: yield the process.");
    suspend_current_task();
    Ok(0)
}

// wait子进程的相关信息
pub enum ChildStatus {
    Success(usize), // 找到了，并且成功回收
    NotZombie, // 找到了，但不是处于僵尸状态
    NotFound, // 没有找到
}

fn waitpid(pid: isize, wstatus: usize) -> ChildStatus {
    let task = get_cpu_local(get_cpu_id()).current_pcb_clone();
    if task.is_none() {
        panic!("No task in schedule!");
    }
    let task_unwrap = task.unwrap();
    let fa_tgid = task_unwrap.tgid;
    let mut ret;
    let mut task_inner = task_unwrap.inner.lock();
    let mut final_idx = task_inner.child.len();
    // 1. 获得 ret 和 final_idx
    if pid < -1 {
        let abs_pid = pid.abs() as usize;
        ret = ChildStatus::NotFound;
        for (idx, child) in task_inner.child.iter().enumerate() {
            if child.tgid == abs_pid {
                ret = ChildStatus::NotZombie;
                if child.is_dead() {
                    ret = ChildStatus::Success(child.pid.value);
                    final_idx = idx;
                    break;
                }
            }
        }
    } else if pid == -1 {
        ret = ChildStatus::NotZombie;
        for (idx, child) in task_inner.child.iter().enumerate() {
            if child.is_dead() {
                ret = ChildStatus::Success(child.pid.value);
                final_idx = idx;
                break;
            }
        }
    } else if pid == 0 {
        ret = ChildStatus::NotFound;
        for (idx, child) in task_inner.child.iter().enumerate() {
            if child.tgid == fa_tgid {
                ret = ChildStatus::NotZombie;
                if child.is_dead() {
                    ret = ChildStatus::Success(child.pid.value);
                    final_idx = idx;
                    break;
                }   
            }
        }
    } else {
        ret = ChildStatus::NotFound;
        for (idx, child) in task_inner.child.iter().enumerate() {
            if child.pid.value == pid as usize {
                ret = ChildStatus::NotZombie;
                if child.is_dead() {
                    ret = ChildStatus::Success(child.pid.value);
                    final_idx = idx;
                    break;    
                }
            }
        }
    }
    // 2. 判断ret的类型
    match ret {
        ChildStatus::Success(_pid) => {
            let child = task_inner.child[final_idx].clone();
            if let Some(code) = child.exit_code() {
                if wstatus != 0 {
                    let _sum = SumGuard::new();
                    unsafe { *(wstatus as *mut i32) = code << 8; }
                }
            }
            drop(child);
            task_inner.child.remove(final_idx);
        },
        // 否则啥也不做
        _ => {},
    }
    ret
}

/* Description: 等待子进程状态的改变，同时获得其信息，并立即返回，否则堵塞。
    pid: (1) < -1: 任意子进程 group id = |pid|
         (2) -1: 任意子进程
         (3) 0: 任意子进程 group id = 父进程的 group id
         (4) >0: 子进程的 pid = pid
    Option：(1) WNOHANG: 如果没有子进程退出，则立即返回，不堵塞。
            (2) WUNTRACED: 子进程已停止（但未通过 ptrace(2) 进行跟踪），也会返回
            (3) WCONTINUED: 如果已停止的子进程已通过发送 SIGCONT 信号而恢复，则也会返回。
    wstatus: 退出码信息(8~15) + core_dump bit(7) + 终止信号信息(0~6)
    注意事项：1.rusage中的内容过于硬核，如果实现则会大大大大大增加系统的复杂度。所以放弃。
        2. 状态的转变包括: terminated \ stop by signal \ resumed by signal
        3. 成功的话，就返回子进程的pid；如果是WNOHANG且子进程没有改变state，则返回0。否则失败，返回-1
 */
pub fn sys_wait4(pid: isize, wstatus: usize, options: usize, _rusage: usize) -> SyscallResult {
    // let _sum = SumGuard::new(); //TODO: 如果在这里添加，则会报错！这是为什么？？？
    let option = Wait4Option::from_bits(options).unwrap();
    info!("[sys_wait4]: pid is {}, wstatus: {:#x}, option: {:?}", pid, wstatus, option);
    loop {
        let ret = waitpid(pid, wstatus);
        match ret {
            ChildStatus::NotZombie => {
                if option.contains(Wait4Option::WNOHANG) {
                    return Ok(0);
                } else {
                    suspend_current_task();
                }
            },
            ChildStatus::NotFound => {
                return Err(Errno::EINVAL);
            },
            ChildStatus::Success(pid) => {
                return Ok(pid);
            }
        }
    }       
}

/* Description: 创建一个子进程，返回线程id，或者 -1
    flags: 1. CLONE_VM：如果设置，则共享地址空间，包括mmap和write等操作。
           2. CLONE_FILES：共享fd_table，任何修改都共享。
                但是，如果其中一个使用了execve，则不再共享，而是留一个副本
                如果没有设置，则子进程得到副本。
           3. CLONE_SIGHAND: 如果设置，共享信号表，但是不共享signal masks 和 等待信号序列。如果未设置，继承副本。
           4. CLONE_THREAD：如果设置，则放入同一线程组；
                当线程组中所有线程终止后，父进程受到SIGCHLD；
                如果线程组中任意线程执行execve，则其他线程终止，并且该线程成为线程组leader
                如果线程使用fork创建子进程，则线程组中的任何线程都可以等待该子进程。
           5. CLONE_PARENT：如果设置，调用进程成为兄弟。如果未设置，调用进程成为父亲。不能在init进程中使用。
           6. CLONE_SETTLS：TLS标识符被设置为 tls
           7. CLONE_PARENT_SETTID：存储子线程tid到父线程地址空间
           8. CLONE_CHILD_SETTID：存储父线程的tid到子进程地址空间
           9. CLONE_CHILD_CLEARTID：清零tid，有助于父线程知道子线程退出，还有个futex??
    stack: 
    tls: 新任务的 tp 值，当包含 CLONE_SETTLS 时设置
    ptid: 当前任务地址空间中的地址，当包含 CLONE_PARENT_SETTID 时，新任务 tid 被存入此处
    ctid: 新任务地址空间中的地址，当包含 CLONE_CHILD_SETTID 时，新任务 tid 被存入此处
    TODO: 这里需要处理一个fork专属的flags，暂时还不知道有什么作用！
*/

pub fn sys_clone(flags: usize, stack: usize, parent_tid: usize, tls: usize, child_tid: usize) -> SyscallResult {
    info!("[sys_clone]: stack: 0x{:x}, parent_tid: {}, tls: {}, child_tid: {}", stack, parent_tid, tls, child_tid);
    // 1. 对参数进行检查
    let _sum = SumGuard::new();
    check_clone_flags(flags as u32);
    let flags = CloneFlags::from_bits_truncate(flags as u32);
    info!("[sys_clone]: flags: {:?}", flags);
    // DONE: 这个stack的地址判断还是有些问题的！目前感觉也不需要做太多的判断，可能会出现问题吧 Unsafe
    let user_stack = match stack {
        0 => None,
        _ => Some(stack),
    };
    if flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
        // TODO：check child_tid 的地址
    } if flags.contains(CloneFlags::CLONE_PARENT_SETTID) {
        // TODO：check parent_tid 的地址
    }
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let new_pid = current_task.from_clone(flags, user_stack, parent_tid, tls, child_tid);
    Ok(new_pid)
}

/* Description: 加载程序，二进制可执行文件 或者 脚本
    注意事项：1. 任何打开的目录流都要关闭
            2. mmap相关的映射要被关闭
            3. signal相关的设置也设置为默认值
            4. 线程组中其他的线程都被破坏
            5. 在调用 execve 之前，确保子进程已经处理了所有待处理的信号，除非你希望信号处理程序在新程序中执行。
            6. 如果 execve 失败，子进程通常应该终止。
    解释：针对于终止线程组中其他线程的操作，在Titanix中的实现思路是：首先无论哪个线程调用execve，都是进程处理
        所以，只需要改变进程的vm即可。同时，做一个断言，即clone时使用了CLONE_THREAD标志，一定会使用CLONE_VM;
        不可能创建一个线程，之后还不共享地址空间。所以这里我们的做法是：（暂时不实现）把calling process之外的
        线程组中的进程给终止，同时继承主进程的pid，成为进程组的leader。
        同时，对于到底是最后保留哪个线程，还是可以保留多个线程，之后写一个程序验证一下或者参考测例中的需求。
 */
pub fn sys_execve(
    path: usize,
    args: usize,
    envs: usize,
) -> SyscallResult {
    /*  1. 解析地址，如果是sh脚本文件，要额外处理
        2. 处理args，envs
        3. 从inode中拿到相关的elf数据
        4. 调用process中的exec函数，加载
            5. 处理地址空间
            6. 终止线程组中的其他线程
            7. 关闭fd_table
            8. 将相关的参数放入用户栈上！
    */
    info!("[sys_execve]: path: 0x{:x}, args: 0x{:x}, envs: {:x}", path, args, envs);
    let _sum = SumGuard::new();

    let path_ptr = path as *const u8;
    let mut args_ptr = args as *const usize;
    let mut envs_ptr = envs as *const usize;
    // TODO: 检查地址的可靠性
    
    let path = ptr_and_dirfd_to_path(AT_FDCWD, path_ptr)?;
    info!("[sys_execve]: path: {:?}", path);
    // TODO：处理可能的 .sh文件
    if path.ends_with(".sh") {
        todo!()
    }
    let dentry = path_to_dentry(&path).ok_or(Errno::ENOENT)?;
    let dentry_lock = dentry.metadata().inner.lock();
    if dentry_lock.d_inode.metadata().i_mode != InodeMode::Regular {
        return Err(Errno::EACCES);
    }
    // TODO: 这里会发生 StorePageFault
    let data = dentry_lock.d_inode.read_all();
    let mut args_vec: Vec<String> = Vec::new();
    let mut envs_vec: Vec<String> = Vec::new();
    loop {
        unsafe {
            if *args_ptr == 0 { break; }
            args_vec.push(c_ptr_to_string((*args_ptr) as *const u8));
            args_ptr = args_ptr.add(1);
        }   
    }
    loop {
        unsafe {
            if *envs_ptr == 0 { break; }
            envs_vec.push(c_ptr_to_string((*envs_ptr) as *const u8));
            envs_ptr = envs_ptr.add(1);
        }   
    }
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    current_task.from_exec(&data, args_vec, envs_vec);
    Ok(0)
}

/* Description: 返回进程(线程)的pid
 */
pub fn sys_gettid() -> SyscallResult {
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    info!("[sys_gettid]: tid(task.pid) is {}", current_task.pid.value);
    Ok(current_task.pid.value)
}

/* Description: 返回进程的ppid
 */
pub fn sys_getppid() -> SyscallResult {
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let current_task_lock = current_task.inner.lock();
    info!("[sys_getppid]: ppid is {}", current_task_lock.ppid);
    Ok(current_task_lock.ppid)
}

/* Description: 返回进程的tgid
 */
pub fn sys_getpid() -> SyscallResult {
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    info!("[sys_getpid]: pid(task.tgid) is {}", current_task.tgid);
    Ok(current_task.tgid)
}