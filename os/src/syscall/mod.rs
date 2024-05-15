use bitflags::bitflags;

use crate::{config::{fs::SECTOR_SIZE, timer::USEC_PER_SEC}, fs::info::{InodeMode, TimeSpec}, timer::current_time_ns};

use self::{fs::{sys_chdir, sys_close, sys_dup, sys_dup3, sys_fstat, sys_getcwd, sys_getdents64, sys_mkdirat, sys_mount, sys_openat, sys_pipe2, sys_read, sys_umount2, sys_uname, sys_unlinkat, sys_write}, mm::{sys_brk, sys_mmap, sys_mprotect, sys_munmap}, process::{sys_clone, sys_execve, sys_exit, sys_getpid, sys_getppid, sys_gettid, sys_wait4, sys_yield}, timer::{sys_gettimeofday, sys_nanosleep, sys_times}};

pub mod error;
mod mm;
mod fs;
mod process;
mod timer;

const SYSCALL_GETCWD: usize = 17;
const SYSCALL_DUP: usize = 23;
const SYSCALL_DUP3: usize = 24;
const SYSCALL_FCNTL: usize = 25;
const SYSCALL_IOCTL: usize = 29;
const SYSCALL_UNLINK: usize = 35;
const SYSCALL_MKNOD: usize = 33;
const SYSCALL_MKDIR: usize = 34;
const SYSCALL_UMOUNT: usize = 39;
const SYSCALL_MOUNT: usize = 40;
const SYSCALL_STATFS: usize = 43;
const SYSCALL_FTRUNCATE: usize = 46;
const SYSCALL_FACCESSAT: usize = 48;
const SYSCALL_CHDIR: usize = 49;
const SYSCALL_FCHMODAT: usize = 53;
const SYSCALL_OPEN: usize = 56;
const SYSCALL_CLOSE: usize = 57;
const SYSCALL_PIPE: usize = 59;
const SYSCALL_GETDENTS: usize = 61;
const SYSCALL_LSEEK: usize = 62;
const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_READV: usize = 65;
const SYSCALL_WRITEV: usize = 66;
const SYSCALL_PREAD64: usize = 67;
const SYSCALL_PWRITE64: usize = 68;
const SYSCALL_SENDFILE: usize = 71;
const SYSCALL_PSELECT6: usize = 72;
const SYSCALL_PPOLL: usize = 73;
const SYSCALL_READLINKAT: usize = 78;
const SYSCALL_NEWFSTATAT: usize = 79;
const SYSCALL_FSTAT: usize = 80;
const SYSCALL_SYNC: usize = 81;
const SYSCALL_FSYNC: usize = 82;
const SYSCALL_UTIMENSAT: usize = 88;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_EXIT_GROUP: usize = 94;
const SYSCALL_SET_TID_ADDRESS: usize = 96;
const SYSCALL_FUTEX: usize = 98;
const SYSCALL_SET_ROBUST_LIST: usize = 99;
const SYSCALL_GET_ROBUST_LIST: usize = 100;
const SYSCALL_NANOSLEEP: usize = 101;
const SYSCALL_SETITIMER: usize = 103;
const SYSCALL_CLOCK_SETTIME: usize = 112;
const SYSCALL_CLOCK_GETTIME: usize = 113;
const SYSCALL_CLOCK_GETRES: usize = 114;
const SYSCALL_CLOCK_NANOSLEEP: usize = 115;
const SYSCALL_SYSLOG: usize = 116;
const SYSCALL_SCHED_SETSCHEDULER: usize = 119;
const SYSCALL_SCHED_GETSCHEDULER: usize = 120;
const SYSCALL_SCHED_GETPARAM: usize = 121;
const SYSCALL_SCHED_SETAFFINITY: usize = 122;
const SYSCALL_SCHED_GETAFFINITY: usize = 123;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_KILL: usize = 129;
const SYSCALL_TKILL: usize = 130;
const SYSCALL_TGKILL: usize = 131;
const SYSCALL_RT_SIGSUSPEND: usize = 133;
const SYSCALL_RT_SIGACTION: usize = 134;
const SYSCALL_RT_SIGPROCMASK: usize = 135;
const SYSCALL_RT_SIGTIMEDWAIT: usize = 137;
const SYSCALL_RT_SIGRETURN: usize = 139;
const SYSCALL_TIMES: usize = 153;
const SYSCALL_SETPGID: usize = 154;
const SYSCALL_GETPGID: usize = 155;
const SYSCALL_SETSID: usize = 157;
const SYSCALL_UNAME: usize = 160;
const SYSCALL_GETRUSAGE: usize = 165;
const SYSCALL_UMASK: usize = 166;
const SYSCALL_GETTIMEOFDAY: usize = 169;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_GETPPID: usize = 173;
const SYSCALL_GETUID: usize = 174;
const SYSCALL_GETEUID: usize = 175;
const SYSCALL_GETGID: usize = 176;
const SYSCALL_GETEGID: usize = 177;
const SYSCALL_GETTID: usize = 178;
const SYSCALL_SYSINFO: usize = 179;
const SYSCALL_SHMGET: usize = 194;
const SYSCALL_SHMCTL: usize = 195;
const SYSCALL_SHMAT: usize = 196;
const SYSCALL_SOCKET: usize = 198;
const SYSCALL_SOCKETPAIR: usize = 199;
const SYSCALL_BIND: usize = 200;
const SYSCALL_LISTEN: usize = 201;
const SYSCALL_ACCEPT: usize = 202;
const SYSCALL_CONNECT: usize = 203;
const SYSCALL_GETSOCKNAME: usize = 204;
const SYSCALL_GETPEERNAME: usize = 205;
const SYSCALL_SENDTO: usize = 206;
const SYSCALL_RECVFROM: usize = 207;
const SYSCALL_SETSOCKOPT: usize = 208;
const SYSCALL_GETSOCKOPT: usize = 209;
const SYSCALL_SHUTDOWN: usize = 210;
const SYSCALL_BRK: usize = 214;
const SYSCALL_MUNMAP: usize = 215;
const SYSCALL_CLONE: usize = 220;
const SYSCALL_EXECVE: usize = 221;
const SYSCALL_MMAP: usize = 222;
const SYSCALL_MPROTECT: usize = 226;
const SYSCALL_MSYNC: usize = 227;
const SYSCALL_MADVISE: usize = 233;
const SYSCALL_WAIT4: usize = 260;
const SYSCALL_PRLIMIT64: usize = 261;
const SYSCALL_REMANEAT2: usize = 276;
const SYSCALL_GETRANDOM: usize = 278;
const SYSCALL_MEMBARRIER: usize = 283;
const SYSCALL_COPY_FILE_RANGE: usize = 285;


pub fn syscall(id: usize, args: [usize; 6]) -> isize {
    let result = match id {
        SYSCALL_WRITE => { sys_write(args[0], args[1], args[2]) },
        SYSCALL_EXIT => { sys_exit(args[0] as i32) },
        SYSCALL_WAIT4 => { sys_wait4(args[0] as isize, args[1], args[2], args[3]) },
        SYSCALL_YIELD => { sys_yield() },
        SYSCALL_CLONE => { sys_clone(args[0], args[1], args[2], args[3], args[4]) },
        SYSCALL_EXECVE => { sys_execve(args[0], args[1], args[2]) },
        SYSCALL_GETPID => { sys_getpid() },
        SYSCALL_GETPPID => { sys_getppid() },
        SYSCALL_GETTID => { sys_gettid() },
        SYSCALL_MUNMAP => { sys_munmap(args[0], args[1]) },
        SYSCALL_MPROTECT => { sys_mprotect(args[0], args[1], args[2] as u32) },
        SYSCALL_MMAP => { sys_mmap(args[0], args[1], args[2] as i32, args[3] as i32, args[4], args[5]) },
        SYSCALL_BRK => { sys_brk(args[0]) }
        SYSCALL_READ => { sys_read(args[0], args[1], args[2])},
        SYSCALL_CHDIR => { sys_chdir(args[0]) },
        SYSCALL_DUP => { sys_dup(args[0]) },
        SYSCALL_DUP3 => { sys_dup3(args[0], args[1], args[2] as u32) },
        SYSCALL_GETCWD => { sys_getcwd(args[0], args[1]) },
        SYSCALL_GETDENTS => { sys_getdents64(args[0], args[1], args[2]) },
        SYSCALL_UNAME => { sys_uname(args[0]) },
        SYSCALL_FSTAT => { sys_fstat(args[0], args[1]) },
        SYSCALL_OPEN => { sys_openat(args[0] as isize, args[1] as *const u8, args[2] as u32, args[3]) },
        SYSCALL_CLOSE => { sys_close(args[0]) },
        SYSCALL_MKDIR => { sys_mkdirat(args[0] as isize, args[1] as *const u8, args[2]) },
        SYSCALL_UNLINK => { sys_unlinkat(args[0] as isize, args[1] as *const u8, args[2] as u32)},
        SYSCALL_MOUNT => { sys_mount(args[0] as *const u8, args[1] as *const u8, args[2] as *const u8, args[3] as u32, args[4])},
        SYSCALL_UMOUNT => { sys_umount2(args[0] as *const u8, args[1]) }
        SYSCALL_GETTIMEOFDAY => { sys_gettimeofday(args[0] as *mut TimeVal, args[1]) }
        SYSCALL_TIMES => { sys_times(args[0] as *mut Tms) }
        SYSCALL_NANOSLEEP => { sys_nanosleep(args[0] as *const TimeSpec, args[1] as *mut TimeSpec) }
        SYSCALL_PIPE => { sys_pipe2(args[0] as *mut i32, args[1] as u32) }
        _ => {
            println!("Unsupported syscall id {}", id);
            Ok(0)
        }
    };

    match result {
        Ok(ret) => ret as isize,
        Err(err) => {
            -(err as isize)
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct STAT {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub __pad1: usize,
    pub st_size: u64,
    pub st_blksize: u32,
    pub __pad2: u32,
    pub st_blocks: u64,
    pub st_atim: TimeSpec,
    pub st_mtim: TimeSpec,
    pub st_ctim: TimeSpec,
}

impl STAT {
    pub fn new() -> Self {
        STAT {
            st_dev: 0,
            st_ino: 0,
            st_mode: 0,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad1: 0,
            st_size: 0,
            st_blksize: SECTOR_SIZE as u32,
            __pad2: 0,
            st_blocks: 0,
            st_atim: TimeSpec::new(),
            st_mtim: TimeSpec::new(),
            st_ctim: TimeSpec::new(),
        }
    }
}


#[repr(C)]
pub struct UtsName {
    /// 系统名称
    pub sysname: [u8; 65],
    /// 网络上的主机名称
    pub nodename: [u8; 65],
    /// 发行编号
    pub release: [u8; 65],
    /// 版本
    pub version: [u8; 65],
    /// 硬件类型
    pub machine: [u8; 65],
    /// 域名
    pub domainname: [u8; 65],
}

impl UtsName {
    pub fn new() -> UtsName {
        UtsName {
            sysname: UtsName::str_to_buf("DuckOS"),
            nodename: UtsName::str_to_buf("DuckMountain"),
            release: UtsName::str_to_buf("1.0.0-generic"),
            version: UtsName::str_to_buf("1.0.0"),
            machine: UtsName::str_to_buf("RISC-V"),
            domainname: UtsName::str_to_buf("Duckos.org"),
        }
    }

    pub fn str_to_buf(sstr: &str) -> [u8; 65] {
        let mut buf: [u8; 65] = [0; 65];
        let mut bytes_buf = sstr.as_bytes().to_vec();
        bytes_buf.resize(buf.len(), 0);
        buf.copy_from_slice(&bytes_buf);
        buf
    }
}

// 本来是有名字的，但是由于这个字段的长度不固定，所以放在外面处理！
#[repr(C)]
#[derive(Debug)]
pub struct Dirent64 {
    d_ino: u64, // inode id
    d_off: i64, // 到下一个dirent的偏移
    d_reclen: u16, // 当前dirent的长度
    d_type: u8, // 文件类型
}

impl Dirent64 {
    pub fn load_dirent64(ino: u64, mode: u8, len: u16) -> Self {
        let dirent = Dirent64 {
            d_ino: ino,
            d_off: 0,
            d_reclen: len,
            d_type: mode,
        };
        dirent
    }

    pub fn dirent_size() -> usize {
        // core::mem::size_of::<Dirent64>() TODO: 不能使用这个计算字节数，实际上这个结构体的字节数是19bytes，但是这个的结果是24bytes
        core::mem::size_of::<u64>() + core::mem::size_of::<i64>() + core::mem::size_of::<u16>() + core::mem::size_of::<u8>()
    }
}

bitflags! {
    #[derive(PartialEq, Debug)]
    pub struct Wait4Option: usize {
        const Block = 0;
        const WNOHANG = 1 << 0;
        const WUNTRACED = 1 << 1;
        const WCONTINUED = 1 << 3;
    }

    /// 用于 sys_clone 的选项
    #[derive(Debug)]
    pub struct CloneFlags: u32 {
        /// fork 专属
        const SIGCHLD = (1 << 4) | (1 << 0);
        /// 共享地址空间
        const CLONE_VM = 1 << 8;
        /// 共享文件系统新信息
        const CLONE_FS = 1 << 9;
        /// 共享文件描述符(fd)表
        const CLONE_FILES = 1 << 10;
        /// 共享信号处理函数
        const CLONE_SIGHAND = 1 << 11;
        /// 创建指向子任务的fd，用于 sys_pidfd_open
        const CLONE_PIDFD = 1 << 12;
        /// 用于 sys_ptrace
        const CLONE_PTRACE = 1 << 13;
        /// 指定父任务创建后立即阻塞，直到子任务退出才继续
        // const CLONE_VFORK = 1 << 14;
        /// 指定子任务的 ppid 为当前任务的 ppid，相当于创建“兄弟”而不是“子女”
        const CLONE_PARENT = 1 << 15;
        /// 作为一个“线程”被创建。具体来说，它同 CLONE_PARENT 一样设置 ppid，且不可被 wait
        const CLONE_THREAD = 1 << 16;
        /// 子任务共享同一组信号量。用于 sys_semop
        const CLONE_SYSVSEM = 1 << 18;
        /// 要求设置 tls
        const CLONE_SETTLS = 1 << 19;
        /// 要求在父任务的一个地址写入子任务的 tid
        const CLONE_PARENT_SETTID = 1 << 20;
        /// 要求将子任务的一个地址清零。这个地址会被记录下来，当子任务退出时会触发此处的 futex
        const CLONE_CHILD_CLEARTID = 1 << 21;
        /// 要求在子任务的一个地址写入子任务的 tid
        const CLONE_CHILD_SETTID = 1 << 24;
    }

    pub struct Dirent64Type: u8 {
        const DT_UNKNOWN = 0;
        const DT_FIFO = 1 << 0;
        const DT_CHR = 1 << 1;
        const DT_DIR = 1 << 2;
        const DT_BLK = 1 << 1 + 1 << 2;
        const DT_REG = 1 << 3;
        const DT_LNK = 1 << 1 + 1 << 3;
        const DT_SOCK = 1 << 2 + 1 << 3;
        const DT_WHT = 1 << 2 +  1 << 1 + 1 << 3;
    }

    pub struct FSFlags: u32 {
        const MS_RDONLY = 1 << 0; //只读挂载文件系统
        const MS_NOSUID = 1 << 1; //禁止设置文件的 SUID 和 SGID 位
        const MS_NODEV = 1 << 2; // 禁止访问设备文件
        const MS_NOEXEC = 1 << 3; // 禁止在文件系统上执行可执行文件
        const MS_SYNCHRONOUS = 1 << 4; // 同步挂载，即对文件系统的写操作立即同步到磁盘
        const MS_REMOUNT = 1 << 5; // 重新挂载文件系统，允许修改挂载标志
        const MS_MANDLOCK = 1 << 6; // 启用强制锁定
        const MS_DIRSYNC = 1 << 7; // 同步目录更新
        const MS_NOATIME = 1 << 10; // 不更新访问时间
        const MS_BIND = 1 << 12; // 绑定挂载，即创建目录或文件的镜像
        const MS_MOVE = 1 << 13; // 原子移动挂载点
    }
}

impl From<InodeMode> for Dirent64Type {
    fn from(value: InodeMode) -> Self {
        match value {
            InodeMode::Block => Dirent64Type::DT_BLK,
            InodeMode::Char => Dirent64Type::DT_CHR,
            InodeMode::Link => Dirent64Type::DT_LNK,
            InodeMode::Regular => Dirent64Type::DT_REG,
            InodeMode::Directory => Dirent64Type::DT_DIR,
            InodeMode::FIFO => Dirent64Type::DT_FIFO,
            InodeMode::Socket => Dirent64Type::DT_SOCK,
        }
    }
}

#[derive(Clone, Copy)]
pub enum FSType {
    VFAT,
    EXT2,
    Proc,
    Dev,
    Tmpfs,
}

impl FSType {
    pub fn str_to_type(s: &str) -> Self {
        match s {
            "vfat" => FSType::VFAT,
            "ext2" => FSType::EXT2,
            "proc" => FSType::Proc,
            "dev" => FSType::Dev,
            "tmpfs" => FSType::Tmpfs,
            _ => panic!("Unsupport file system type!"),
        }
    }
}

#[repr(C)]
#[derive(PartialEq, PartialOrd)]
pub struct TimeVal {
    pub tv_sec: usize, /* seconds */
    pub tv_usec: usize, /* microseconds */
}

impl TimeVal {
    pub fn now() -> Self {
        let usec = current_time_ns();
        Self {
            tv_sec: usec / USEC_PER_SEC,
            tv_usec: usec % USEC_PER_SEC,
        }
    }
}

#[repr(C)]
pub struct Tms {
    pub utime: usize,
    pub stime: usize,
    pub cutime: usize,
    pub cstime: usize,
}