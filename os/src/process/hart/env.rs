//！ 每个核中的环境信息

use core::arch::asm;

use riscv::register::sstatus;

use super::cpu::{get_cpu_id, get_cpu_local};

pub struct Env {
    sum_status: usize,
}

impl Env {
    pub const fn empty() -> Self {
        Self {
            sum_status: 0,
        }
    }

    pub fn sum_on(&mut self) {
        if self.sum_status == 0 {
            unsafe {
                sstatus::set_sum();
            }
        } else if !self.read_sum() {
            // 有时候在切换任务的时候，会发生系统调用还没运行结束，结果sum位就被硬件磨除了；
            self.sum_status = 0;
            unsafe {
                sstatus::set_sum();
            }
        }
        self.sum_status += 1;
    }

    pub fn sum_off(&mut self) {
        if self.sum_status == 1 {
            unsafe {
                sstatus::clear_sum();
            }
        }
        self.sum_status -= 1;
    }

    fn read_sum(&self) -> bool {
        let sstatus: usize;
        unsafe {
            asm!("csrr {}, sstatus", out(reg) sstatus);
        }
        (sstatus & (1 << 18)) == 1
    }
}

pub struct SumGuard {}

impl SumGuard {
    pub fn new() -> Self {
        let cpu_id = get_cpu_id();
        get_cpu_local(cpu_id).env.lock().sum_on();
        Self {}
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        let cpu_id = get_cpu_id();
        get_cpu_local(cpu_id).env.lock().sum_off();
    }
}
