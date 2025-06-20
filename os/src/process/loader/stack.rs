//！ 处理user_stack中的信息
// 参考资料：https://github.com/chenpengcong/blog/issues/18

use alloc::{string::String, vec::Vec};
use log::{debug, info};
use xmas_elf::ElfFile;
use crate::{config::mm::PAGE_SIZE, process::hart::env::SumGuard, utils::random::RANDOM_GENERATOR};

pub struct StackInfo {
    auxv: Vec<(usize, usize)>,
    args: Vec<String>,
    envs: Vec<String>,
}

impl StackInfo {
    pub fn empty() -> Self {
        Self {
            auxv: Vec::new(),
            args: Vec::new(),
            envs: Vec::new(),
        }
    }

    pub fn init_arg(&mut self, args: Vec<String>, envs: Vec<String>) {
        self.args = args;
        self.envs = envs;
    }

    pub fn init_auxv(&mut self, elf: &ElfFile) {
        // 计算elf_head_addr，用来赋值AT_PHDR
        let mut elf_head_addr: usize = 0;
        for i in 0..elf.header.pt2.ph_count() {
            if elf.program_header(i)
                .unwrap()
                .get_type()
                .unwrap() == xmas_elf::program::Type::Load {
                elf_head_addr = 
                    elf.program_header(i)
                    .unwrap().virtual_addr() as usize;
                break;
            }
        }
        self.auxv.push((AT_IGNORE, 0));
        self.auxv.push((AT_PHDR, elf_head_addr + elf.header.pt2.ph_offset() as usize));
        self.auxv.push((AT_PHENT, elf.header.pt2.ph_entry_size() as usize));
        self.auxv.push((AT_PHNUM, elf.header.pt2.ph_count() as usize));
        self.auxv.push((AT_PAGESZ, PAGE_SIZE));
        self.auxv.push((AT_FLAGS, 0 as usize));
        self.auxv.push((AT_ENTRY, elf.header.pt2.entry_point() as usize));
        // self.auxv.push((AT_NOTELF, 0 as usize)); TODO：先不设置，也不知道设置什么值比较好！
        self.auxv.push((AT_UID, 0 as usize));
        self.auxv.push((AT_GID, 0 as usize));
        self.auxv.push((AT_EGID, 0 as usize));
        self.auxv.push((AT_HWCAP, 0 as usize));
        self.auxv.push((AT_CLKTCK, 100 as usize));
        self.auxv.push((AT_SECURE, 0 as usize));
        
    }

    pub fn set_auxv_at_base(&mut self, value: usize) {
        self.auxv.push((AT_BASE, value));
    }

    pub fn set_auxv_at_random(&mut self, value: usize) {
        self.auxv.push((AT_RANDOM, value));
    }

    pub fn set_auxv_at_null(&mut self, value: usize) {
        self.auxv.push((AT_NULL, value));
    }

    pub fn set_auxv_at_execfn(&mut self, value: usize) {
        self.auxv.push((AT_EXECFN, value));
    }

    pub fn set_auxv_at_platform(&mut self, value: usize) {
        self.auxv.push((AT_PLATFORM, value));
    }

    // TODO： 这里的 argv_addr 采用的是push，所以第一个argument的地址放在第一个，不知道有没有问题？？？？
    pub fn build_stack(&mut self, ustack_sp: usize) -> (usize, StackLayout) {
        let _sum = SumGuard::new();
        let mut sp = ustack_sp;
        let args_len = self.args.len();
        let envs_len = self.envs.len();
        let mut argv_addr: Vec<usize> = Vec::with_capacity(args_len);
        let mut envp_addr: Vec<usize> = Vec::with_capacity(envs_len);

        // construct envp str
        for i in 0..envs_len {
            // 最后一个字节为0,标识结束
            sp -= 1;
            let end_ptr = sp as *mut u8;
            sp -= self.envs[i].len();
            let ptr = sp as *mut u8;
            envp_addr.push(sp);
            
            unsafe {
                debug!(
                    "[stack.rs] envp i:{} ptr addr 0x{:x}, end_ptr addr 0x{:x}, envs_len: {}",
                    i, ptr as usize, end_ptr as usize, self.envs[i].len() as usize
                );
                core::ptr::copy_nonoverlapping(
                    self.envs[i].as_ptr(), ptr, self.envs[i].len()
                );
                *end_ptr = 0;
            }
        }
        // 这里我们以16字节对齐 sp指针
        sp &= !(core::mem::size_of::<usize>() * 2 - 1);

        // construct argument str
        for i in 0..args_len {
            // 最后一个字节为0,标识结束
            sp -= 1;
            let end_ptr = sp as *mut u8;
            sp -= self.args[i].len();
            let ptr = sp as *mut u8;
            argv_addr.push(sp);
            unsafe {
                debug!(
                    "[stack.rs] argu i:{} ptr addr 0x{:x}, end_ptr addr 0x{:x}, envs_len: {}",
                    i, ptr as usize, end_ptr as usize, self.args[i].len() as usize
                );
                core::ptr::copy_nonoverlapping(
                    self.args[i].as_ptr(), ptr, self.args[i].len()
                );
                *end_ptr = 0;
            }
        }
        // padding for align
        // 这里我们以16字节对齐 sp指针
        sp &= !(core::mem::size_of::<usize>() * 2 - 1);

        // 载入platform
        let platform = b"riscv64\0";
        sp = sp - platform.len();

        self.set_auxv_at_platform(sp);
        let ptr = sp as *mut u8;
        unsafe {
            debug!(
                "[stack.rs] platform: ptr addr 0x{:x}",
                ptr as usize
            );
            core::ptr::copy_nonoverlapping(
                platform.as_ptr(), ptr, platform.len()
            );
        }
        
        // 载入random bytes
        sp -= core::mem::size_of::<usize>() * 2;
        self.set_auxv_at_random(sp);
        let ptr = sp as *mut u8;
        let mut random_bytes = [0u8; 16];
        let mut rng = RANDOM_GENERATOR.lock();
        for i in 0..4 {
            random_bytes[i*4..(i+1)*4].copy_from_slice(&(rng.genrand_u32() as u32).to_be_bytes());
        }
        drop(rng);
        unsafe {
            debug!(
                "[stack.rs] random bytes: ptr addr 0x{:x}",
                ptr as usize
            );
            core::ptr::copy_nonoverlapping(random_bytes.as_ptr(), ptr, 16);
        }
        // padding 对齐16字节
        sp &= !(core::mem::size_of::<usize>() * 2 - 1);

        // construct auxv
        let execfn = argv_addr.first().copied().unwrap_or(0);
        self.set_auxv_at_execfn(execfn);
        self.set_auxv_at_null(0);
        let auxv_size = core::mem::size_of::<usize>() * 2;
        let auxv_space = self.auxv.len() * auxv_size;
        sp -= auxv_space;
        let auxv_0 = sp; // 记录下第一个的位置
        for (i, &(key, value)) in self.auxv.iter().enumerate() {
            let ptr = (sp + i * auxv_size) as *mut usize;
            unsafe {
                *ptr.offset(0) = key;
                *ptr.offset(1) = value;
            }
        }

        // construct envp pointer
        let envp_space = envp_addr.len() * core::mem::size_of::<usize>();
        sp -= core::mem::size_of::<usize>();
        unsafe {
            *(sp as *mut usize) = 0; // 代表着 envp[term] = NULL
        }
        sp -= envp_space;
        let envp_0 = sp;
        for i in 0..envp_addr.len() {
            let ptr = (sp + i*core::mem::size_of::<usize>()) as *mut usize;
            unsafe {
                *ptr = envp_addr[i];
            }
        }
        // construct argv pointer
        let argv_space = argv_addr.len() * core::mem::size_of::<usize>();
        sp -= core::mem::size_of::<usize>();
        unsafe {
            *(sp as *mut usize) = 0; // 代表着 argv[term] = NULL
        }
        sp -= argv_space;
        let argv_0 = sp;
        for i in 0..argv_addr.len() {
            let ptr = (sp + i*core::mem::size_of::<usize>()) as *mut usize;
            unsafe {
                *ptr = argv_addr[i];
            }
        }
        // construct argc
        sp -= core::mem::size_of::<usize>();
        let argc_0 = sp;
        unsafe {
            *(sp as *mut usize) = self.args.len() as usize;
        }
        // TODO：不太确定这里的sp是否还要对齐到16字节 ？？？？？
        debug!(
            "[stack.rs] argc_0: 0x{:x}, argv_0: 0x{:x}, envp_0: 0x{:x}, auxv_0: 0x{:x}",
            argc_0, argv_0, envp_0, auxv_0
        );
        (sp, StackLayout::new(argc_0, argv_0, envp_0, auxv_0))
    }

}

// 存放着栈上argc argv envp auxv的地址
pub struct StackLayout {
    pub argc: usize,
    pub argv: usize,
    pub envp: usize,
    pub auxv: usize,
}

impl StackLayout {
    pub fn new(argc: usize, argv: usize, envp: usize, auxv: usize) -> Self {
        Self { argc, argv, envp, auxv, }
    }

    pub fn empty() -> Self {
        Self { argc: 0, argv: 0, envp: 0, auxv: 0 }
    }
}

/// 查看auxv的常用值 /proc/self/auxv 文件
/// End of vector
pub const AT_NULL: usize = 0;
/// Entry should be ignored
pub const AT_IGNORE: usize = 1;
/// File descriptor of program
pub const AT_EXECFD: usize = 2;
/// Program headers for program
pub const AT_PHDR: usize = 3;
/// Size of program header entry
pub const AT_PHENT: usize = 4;
/// Number of program headers
pub const AT_PHNUM: usize = 5;
/// System page size
pub const AT_PAGESZ: usize = 6;
/// Base address of interpreter
pub const AT_BASE: usize = 7;
/// Flags
pub const AT_FLAGS: usize = 8;
/// Entry point of program
pub const AT_ENTRY: usize = 9;
/// Program is not ELF
pub const AT_NOTELF: usize = 10;
/// Real uid
pub const AT_UID: usize = 11;
/// Effective uid
pub const AT_EUID: usize = 12;
/// Real gid
pub const AT_GID: usize = 13;
/// Effective gid
pub const AT_EGID: usize = 14;
/// String identifying platform.
pub const AT_PLATFORM: usize = 15;
/// Machine-dependent hints about processor capabilities.
pub const AT_HWCAP: usize = 16;
/// Frequency of times() 
pub const AT_CLKTCK: usize = 17;
/// Used FPU control word.
pub const AT_FPUCW: usize = 18;
/// Data cache block size.
pub const AT_DCACHEBSIZE: usize = 19;
/// Instruction cache block size.
pub const AT_ICACHEBSIZE: usize = 20;
/// Unified cache block size.
pub const AT_UCACHEBSIZE: usize = 21;
/// Entry should be ignored.
pub const AT_IGNOREPPC: usize = 22;
/// Boolean, was exec setuid-like?
pub const AT_SECURE: usize = 23;
/// String identifying real platforms.
pub const AT_BASE_PLATFORM: usize = 24;
/// Address of 16 random bytes.
pub const AT_RANDOM: usize = 25;
/// More machine-dependent hints about processor capabilities.
pub const AT_HWCAP2: usize = 26;
/// Filename of executable.
pub const AT_EXECFN: usize = 31;
/// Pointer to the global system page used for system calls and other nice things.
pub const AT_SYSINFO: usize = 32;
/// Pointer to the global system page used for system calls and other nice things.
pub const AT_SYSINFO_EHDR: usize = 33;
