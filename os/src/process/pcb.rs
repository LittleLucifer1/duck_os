//! 进程信息控制块

use alloc::{sync::Weak, string::{String, ToString}, sync::Arc, vec::Vec};

use crate::{fs::fd_table::FdTable, mm::memory_set::mem_set::MemeorySet, sync::SpinLock, syscall::CloneFlags};

use super::{context::TaskContext, kstack::Kstack, loader::load_elf, pid::{alloc_pid, Pid}, schedule::push_task_to_schedule, trap::context::{Register, TrapContext}};

pub struct PCB {
    // 进程相关
    pub tgid: usize, // 组标识符，外部可见的pid
    pub pid: Pid, // 唯一标识符，内部可见的pid
    pub kernel_stack: Kstack,
    pub vm: Arc<SpinLock<MemeorySet>>,
    pub fd_table: Arc<SpinLock<FdTable>>,
    pub inner: Arc<SpinLock<PCBInner>>,
}

pub struct PCBInner {
    pub cwd: String,
    pub ppid: usize,
    pub exit_code: i32,
    pub parent: Option<Weak<PCB>>,
    pub child: Vec<Arc<PCB>>,
    pub task_cx: TaskContext,
    pub status: TaskStatus,
}

unsafe impl Send for PCBInner {}

impl PCB {
    pub fn elf_data_to_pcb(cwd: &str, data: &[u8]) -> Self {
        let mut vm = MemeorySet::new_user();
        let (entry_point, user_stack, _) = load_elf(data, &mut vm, Vec::new(), Vec::new());
        let kernel_stack = Kstack::init_kernel_stack();
        let ks_top = kernel_stack.push_trap_cx(TrapContext::init_trap_cx(entry_point, user_stack));
        let pid = alloc_pid().unwrap();
        
        Self {
            tgid: pid.value,
            pid,
            kernel_stack,
            vm: Arc::new(SpinLock::new(vm)),
            fd_table: Arc::new(SpinLock::new(FdTable::init_fdtable())),
            inner: Arc::new(SpinLock::new(PCBInner {
                cwd: cwd.to_string(),
                ppid: 1,
                exit_code: 0,
                parent: None,
                child: Vec::new(),
                task_cx: TaskContext::init_task_cx(ks_top),
                status: TaskStatus::Ready,
            }))
        }
    }

    // 默认相关的地址已经检查好了，只要需要使用，都是正确的地址。同时负责加载到schedule中
    pub fn from_clone(
        self: &Arc<PCB>, 
        flags: CloneFlags, 
        user_stack: Option<usize>,
        parent_tid: usize,
        tls: usize,
        child_tid: usize,
    ) -> usize {
        // 1. 共享地址空间
        let vm = if flags.contains(CloneFlags::CLONE_VM) {
            Arc::clone(&self.vm)
        } else {
            Arc::new(SpinLock::new(self.vm.lock().from_user()))
        };
        // 2. 共享 fd_table
        let fd_table = if flags.contains(CloneFlags::CLONE_FILES) {
            Arc::clone(&self.fd_table)
        } else {
            Arc::new(SpinLock::new(self.fd_table.lock().from_clone_copy()))
        };
        let mut inner_lock = self.inner.lock();
        let pid = alloc_pid().unwrap();
        let pid_value = pid.value;
        // 3. 处理 ppid 和 tgid
        let ppid = if flags.contains(CloneFlags::CLONE_PARENT) {
            inner_lock.ppid
        } else {
            self.pid.value
        };
        let tgid = if flags.contains(CloneFlags::CLONE_THREAD) {
            self.tgid
        } else {
            pid.value
        };
        // 4. 处理 tls, ptid, ctid

        // 5. 处理内核栈和 trap_cx
        let kernel_stack = Kstack::init_kernel_stack();
        let tx = self.kernel_stack.top_trap_cx();
        let mut trap_cx = TrapContext::empty();
        trap_cx.from_clone(tx);
        // 5.1 处理其中的部分寄存器
        trap_cx.set_register(Register::a0, 0);
        if flags.contains(CloneFlags::CLONE_SETTLS) {
            trap_cx.set_register(Register::tp, tls);
        }
        // TODO: 没有处理 CLONE_CHILD_SETTID、CLONE_CHILD_CLEARTID参数
        if let Some(user_stack) = user_stack {
            trap_cx.set_register(Register::sp, user_stack);
        }
        let stack_top = kernel_stack.push_trap_cx(trap_cx);
        // 6. 构建新的pcb
        let new_pcb = Arc::new(PCB {
            tgid,
            pid,
            kernel_stack,
            vm,
            fd_table,
            inner: Arc::new(SpinLock::new(PCBInner {
                cwd: inner_lock.cwd.clone(),
                ppid,
                exit_code: 0,
                parent: if flags.contains(CloneFlags::CLONE_PARENT) {
                    if inner_lock.parent.is_some() {
                        Some(Weak::clone(&inner_lock.parent.as_ref().unwrap()))
                    } else {
                        None
                    }
                } else {
                    Some(Arc::downgrade(&self))
                },
                child: Vec::new(),
                task_cx: TaskContext::init_task_cx(stack_top),
                status: TaskStatus::Ready,
            }))
        });
        // 7. 处理关系
        if !flags.contains(CloneFlags::CLONE_PARENT) {
            inner_lock.child.push(Arc::clone(&new_pcb));
        }
        push_task_to_schedule(Arc::clone(&new_pcb));
        pid_value
    }

    /* Function: 将当前的进程修改为另一个进程。
       TODO：不完善，缺少很多细节 */
    pub fn from_exec(&self, data: &[u8], args_vec: Vec<String>, envs_vec: Vec<String>) {
        self.vm.lock().clear_user_space();
        self.fd_table.lock().close_exec();
        // TODO：清空信号模块
        // TODO：清空时间统计
        // TODO：关闭其他的线程组
        
        let mut vm_lock = self.vm.lock();
        let (entry, user_stack, stack_layout) = load_elf(data, &mut vm_lock, args_vec, envs_vec);
        vm_lock.activate();
        let kernel_stack = self.kernel_stack.push_trap_cx(
            TrapContext::exec_trap_cx(entry, user_stack, stack_layout.unwrap())
        );
        self.inner.lock().task_cx = TaskContext::init_task_cx(kernel_stack);
    }

    pub fn task_cx_ptr(&self) -> *const TaskContext {
        let inner = self.inner.lock();
        &inner.task_cx
    }

    pub fn set_status(&self, status: TaskStatus) {
        self.inner.lock().status = status;
    }

    pub fn status(&self) -> TaskStatus {
        self.inner.lock().status
    }

    pub fn set_exit_code(&self, exit_code: i32) {
        self.inner.lock().exit_code = exit_code;
    }

    pub fn clear_child(&self) {
        self.inner.lock().child.clear();
    }

    pub fn exit_code(&self) -> Option<i32> {
        let inner = self.inner.try_lock()?;
        match inner.status {
            TaskStatus::Dead => Some(inner.exit_code),
            _ => None,
        }
    }

    pub fn is_dead(&self) -> bool {
        // 如果拿不到锁，说明此时这个进程肯定不是Dead,返回None
        let inner = self.inner.try_lock();
        if let Some(inner_unwrap) = inner {
            inner_unwrap.status == TaskStatus::Dead
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum TaskStatus {
    Running, // 运行状态
    Ready, // 就绪状态
    Dead, // 还没被父进程回收
    Interruptible, // 等待某些事件发生，会被挂起
    Exit, // 已经被回收了
}