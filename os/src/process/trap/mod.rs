use core::{arch::global_asm, panic};

use log::info;
use riscv::register::{mtvec, scause::{self, Exception, Trap}, sstatus, stval, stvec};

use crate::{process::hart::cpu::get_cpu_id, syscall::syscall};

use self::context::TrapContext;

use super::hart::cpu::get_cpu_local;

pub mod context;

global_asm!(include_str!("trap.S"));

pub fn init_stvec() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        // TODO: 目前使用大表里面写分支, 或者后续可以写一个中断向量表
        stvec::write(__alltraps as usize, mtvec::TrapMode::Direct);
    }
}

#[no_mangle]
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    match sstatus::read().spp() {
        sstatus::SPP::Supervisor => kernel_trap_handler(cx),
        sstatus::SPP::User => user_trap_handler(cx),
    }
}

#[no_mangle]
pub fn user_trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            let num = syscall(
                cx.x[17],
                [cx.x[10], cx.x[11], cx.x[12], cx.x[13], cx.x[14], cx.x[15]],
            ) as usize;
            cx.set_register(context::Register::a0, num);
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
            let vm_lock = current_task.vm.lock();
            match vm_lock.handle_page_fault(stval, scause) {
                Ok(_) => {
                    info!("[page_fault_handler]: handle legal page fault, addr: 0x{:x}, instruction: 0x{:x}",
                        stval, cx.sepc
                    );
                },
                Err(_) => {
                    info!("[page_fault_handler]: Encounter illegal page fault, addr: 0x{:x}, instruction: 0x{:x}, scause: {:?}",
                        stval, cx.sepc, scause.cause(),
                    );
                    panic!();
                    // TODO: 之后要相关的进程要收到信号 SIGSEGV（段错误）
                }
            }
        }
        _ => {
            println!("[cpu {}] Unsupported trap {:?}, stval = {:#x}", get_cpu_id(), scause.cause(), stval);
            panic!();
        }
    }
    cx
}

#[no_mangle]
pub fn kernel_trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let _stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
                       
        }
        _ => {
            // info!(
            //     "[kernel] {:?}(scause:{}) in application, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
            //     scause::read().cause(),
            //     scause::read().bits(),
            //     stval::read(),
            //     sepc::read(),
            // );
            // panic!(
            //     "a trap {:?} from kernel! stval {:#x}",
            //     scause::read().cause(),
            //     stval::read()
            // );
        }
    }
    cx
}