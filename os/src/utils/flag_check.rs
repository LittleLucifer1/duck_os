
use crate::syscall::CloneFlags;



// 当前只实现了9个flags
pub fn check_clone_flags(flags: u32) {
    let valid_flag = 
        CloneFlags::SIGCHLD |
        CloneFlags::CLONE_VM | 
        CloneFlags::CLONE_FILES | 
        CloneFlags::CLONE_PARENT | 
        CloneFlags::CLONE_THREAD |
        CloneFlags::CLONE_SETTLS |
        CloneFlags::CLONE_PARENT_SETTID |
        CloneFlags::CLONE_CHILD_CLEARTID |
        CloneFlags::CLONE_CHILD_SETTID |
        CloneFlags::CLONE_SIGHAND;
    if (flags | valid_flag.bits()) != valid_flag.bits() {
        panic!("Unsupport Clone flags: {}, flags: {}, valid flags: {}", flags | valid_flag.bits(), flags, valid_flag.bits());
    }
}