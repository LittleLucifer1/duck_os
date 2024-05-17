use core::cell::{RefCell, RefMut};

use riscv::register::sstatus;

use crate::config::task::MAX_CORE_NUM;

fn cpu_id() -> u8 {
    let mut cpu_id;
    unsafe {
        core::arch::asm!("mv {0}, tp", out(reg) cpu_id);
    }
    cpu_id
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(align(64))]
pub struct Cpu {
    pub noff: i32,              // Depth of push_off() nesting.
    pub interrupt_enable: bool, // Were interrupts enabled before push_off()?
}

impl Cpu {
    const fn new() -> Self {
        Self {
            noff: 0,
            interrupt_enable: false,
        }
    }
}

pub struct SafeRefCell<T>(RefCell<T>);

// #Safety: Only the corresponding cpu will access it.
unsafe impl<Cpu> Sync for SafeRefCell<Cpu> {}

impl<T> SafeRefCell<T> {
    const fn new(t: T) -> Self {
        Self(RefCell::new(t))
    }
}

// Avoid hard code
#[allow(clippy::declare_interior_mutable_const)]
const DEFAULT_CPU: SafeRefCell<Cpu> = SafeRefCell::new(Cpu::new());

static CPUS: [SafeRefCell<Cpu>; MAX_CORE_NUM] = [DEFAULT_CPU; MAX_CORE_NUM];

pub fn mycpu() -> RefMut<'static, Cpu> {
    CPUS[cpu_id() as usize].0.borrow_mut()
}

// 关中断
pub fn push_off() {
    let old_sie = sstatus::read().sie();
    unsafe {
        sstatus::clear_sie();
    }
    let mut cpu = mycpu();
    if cpu.noff == 0 {
        cpu.interrupt_enable = old_sie;
    }
    cpu.noff += 1;
}

// 开中断
// 连续两个 off, 如果再on的话，则只能抵消后一个off，所以此时还是off状态。
pub fn push_on() {
    let mut cpu = mycpu();
    if sstatus::read().sie() || cpu.noff < 1 {
        todo!()
    }
    cpu.noff -= 1;
    let should_enable = cpu.noff == 0 && cpu.interrupt_enable;
    drop(cpu);
    if should_enable {
        unsafe { sstatus::set_sie(); }
    }
}

