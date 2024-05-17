use riscv::register::sstatus::{self, Sstatus, SPP};

use crate::process::{hart::cpu::get_cpu_id, loader::stack::StackLayout};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapContext {
    pub x: [usize; 32], // 0 ~ 31
    pub sstatus: Sstatus, // 32
    pub sepc: usize, // 33
    // 这里的cpu_id很特殊。一般来说，从用户态回到内核态，只需要把用户态的寄存器保存好即可。
    // 但是因为用户态和内核态都会使用这个tp寄存器，所以需要额外保存，用于 u->s 时恢复tp的值
    pub cpu_id: usize, // 34

    // TODO: 修改这里东西的时候，要修改trap.S文件中的数值
}

impl TrapContext {
    pub fn empty() -> Self {
        Self {
            x: [0; 32],
            sstatus: sstatus::read(),
            sepc: 0,
            cpu_id: 0,
        }
    }

    pub fn set_register(&mut self, reg: Register, value: usize) {
        let idx: usize = reg.into();
        self.x[idx] = value;
    }

    // 初始化trap_cx，用于任务第一次回到用户态
    pub fn init_trap_cx(entry: usize, sp: usize) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            cpu_id: get_cpu_id(),
        };
        cx.set_register(Register::sp, sp);
        cx
    }

    pub fn exec_trap_cx(entry: usize, sp: usize, stack_layout: StackLayout) -> Self {
        let mut cx = TrapContext::init_trap_cx(entry, sp);
        cx.set_register(Register::a0, stack_layout.argc);
        cx.set_register(Register::a1, stack_layout.argv);
        cx.set_register(Register::a2, stack_layout.envp);
        cx.set_register(Register::a3, stack_layout.auxv);
        cx
    }

    // 复制trap_context中的内容
    pub fn from_clone(&mut self, tx :&Self) {
        self.x.copy_from_slice(&tx.x);
        self.sstatus = tx.sstatus;
        self.sepc = tx.sepc;
        self.cpu_id = tx.cpu_id;
    }
}

#[allow(non_camel_case_types)]
pub enum Register {
    zero = 0,
    ra,sp,gp,tp,
    t0,t1,t2,
    fp,s1,
    a0,a1,a2,a3,a4,a5,a6,a7,
    s2,s3,s4,s5,s6,s7,s8,s9,s10,s11,
    t3,t4,t5,t6
}

impl From<Register> for usize {
    fn from(reg: Register) -> usize {
        match reg {
            Register::zero => 0,
            Register::ra => 1,
            Register::sp => 2,
            Register::gp => 3,
            Register::tp => 4,
            Register::t0 => 5,
            Register::t1 => 6,
            Register::t2 => 7,
            Register::fp => 8,
            Register::s1 => 9,
            Register::a0 => 10,
            Register::a1 => 11,
            Register::a2 => 12,
            Register::a3 => 13,
            Register::a4 => 14,
            Register::a5 => 15,
            Register::a6 => 16,
            Register::a7 => 17,
            Register::s2 => 18,
            Register::s3 => 19,
            Register::s4 => 20,
            Register::s5 => 21,
            Register::s6 => 22,
            Register::s7 => 23,
            Register::s8 => 24,
            Register::s9 => 25,
            Register::s10 => 26,
            Register::s11 => 27,
            Register::t3 => 28,
            Register::t4 => 29,
            Register::t5 => 30,
            Register::t6 => 31,
        }
    }
}
