use alloc::{string::{String, ToString}, vec::Vec};

use crate::{fs::AT_FDCWD, process::hart::cpu::{get_cpu_id, get_cpu_local}, syscall::error::{Errno, OSResult}};

use super::string::c_ptr_to_string;
/*
    路径相关的功能函数：
    1. path的最后一个name
    2. path的 parent path
    3. format 相对路径，即去掉前缀../ 和 ./
    4. 相对路径path + cwd -> 绝对路径
    5. cwd + name -> 绝对路径
    6. format原始路径，去掉 \t \n等，还有最后一个 “/”
*/

// 得到路径上最后一个name
pub fn dentry_name(path: &str) -> &str {
    if path == "" {
        return "";
    }
    let names: Vec<&str> = path.split('/').filter(|name| *name != "").collect();
    names[names.len() - 1]
}

// Assumption: path 只为 / 开头的路径 或者 ../ ./ 开头的合法路径
pub fn is_relative_path(path: &str) -> bool {
    if path.starts_with("/") {
        false
    } else {
        true
    }
}

// Assumption: 此时的 path 已经是合法的绝对路径，同时也是format路径，找到它的父路径
// 返回的形式 / or /xxx
pub fn parent_path(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    } else {
        let mut names: Vec<&str> = path.split("/").filter(|name| *name != "" ).collect();
        if names.len() == 1 {
            return "/".to_string();
        } else {
            names.insert(0, "");
            names.pop();
            names.join("/")
        }
    }
}

// Assumption: 此时的 path 是合法的路径： ../ 和 ./
// 去除掉 ../ 和 ./
fn format_rel_path(path: &str) -> String {
    path.trim_start_matches(|c| c == '.' || c == '/').to_string()
} 

// 相对路径 ---> 绝对路径 由cwd -> abs_path
// Assumption： path已经是相对路径 ../ 或者 ./ 且 cwd已经是 / or /xxx/yyy
// 返回形式: /xxx/yy 或者 / 
pub fn cwd_and_path(path: &str, cwd: &str) -> String {
    if path.starts_with("../") {
        let mut cwd_pa = parent_path(cwd);
        let f_path = format_rel_path(path);
        if !&cwd_pa.ends_with("/") {
            cwd_pa.push('/');
        }
        cwd_pa.push_str(&f_path);
        cwd_pa
    } else if path.starts_with("./") {
        let mut cwd: String = cwd.to_string();
        let f_path = format_rel_path(path);
        if !&cwd.ends_with("/") {
            cwd.push('/');
        }
        cwd.push_str(&f_path);
        cwd
    } else {
        let mut cwd: String = cwd.to_string();
        let f_path = format_rel_path(path);
        if !&cwd.ends_with("/") {
            cwd.push('/');
        }
        cwd.push_str(&f_path);
        cwd
    }
}

// cwd + name ---> 绝对路径
// Assumption： name中没有 / 且 cwd已经是 / or /xxx/yyy
// 返回形式: /xxx/yy 或者 / 
pub fn cwd_and_name(name: &str, cwd: &str) -> String {
    if name.contains("/") {
        panic!("File or directory name has /");
    }
    let mut cwd = cwd.to_string();
    if !&cwd.ends_with("/") {
        cwd.push('/');
    }
    cwd.push_str(name);
    cwd
}

// 规范化路径，主要是去掉 \t \n等，还有最后一个 “/”
pub fn format_path(path: &str) -> String {
    if path == "" {
        return "".to_string();
    } else {
        path.trim_end_matches(|c| c == '/' || c == '\t' || c == '\n').to_string()
    }
}

// // Assumption: 通过dirfd找对应的path  dirfd可能是AT_FDCWD
// // 返回对应的format path
fn dirfd_to_path(dirfd: isize) -> OSResult<String> {
    if dirfd == AT_FDCWD {
        return Ok(get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap().inner.lock().cwd.clone());
    }
    let fds = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap().fd_table.clone();
    
    let fds_lock = fds.lock();
    let fd_info = fds_lock.fd_table.get(&(dirfd as usize)).ok_or(Errno::EBADF)?;
    let path = fd_info.file.metadata().f_dentry.metadata().inner.lock().d_path.clone();
    Ok(path)
}

// // 相对路径 ---> 绝对路径 暂时先命名为dirfd_and_path
// // Assumption： path已经是相对路径 ../ 或者 ./
// // 返回形式: /xxx/yy 或者 / 
pub fn dirfd_and_path(dirfd: isize, path: &str) -> OSResult<String> {
    let format_path = format_path(path);
    let format_dirfd = dirfd_to_path(dirfd)?;
    // path is ../
    if format_path.starts_with("../") {
        let mut cwd = parent_path(&format_dirfd);
        let end_path = format_rel_path(&format_path);
        if !cwd.ends_with('/') {
            cwd.push('/');
        }
        cwd.push_str(&end_path);
        Ok(cwd)
    } else {
        // path is ./ or name(no ./ or ../)
        let mut cwd = format_dirfd;
        let end_path = format_rel_path(&format_path);
        if !cwd.ends_with('/') {
            cwd.push('/');
        }
        cwd.push_str(&end_path);
        Ok(cwd)
    }
}

// // Assumption: 暂时用在execve, openat中，dirfd默认是AT_FDCWD
// // 如果path为相对地址，则dirfd是目前的进程地址
// // 如果path为绝对地址，则dirfd没有用
// // path为NULL，或者地址不存在都会报错
pub fn ptr_and_dirfd_to_path(dirfd: isize, ptr: *const u8) -> OSResult<String> {
    match ptr as usize {
        0 => {
            // TODO: 这里应该要返回syscall类型的错误
            panic!()
        }
        _ => {
            let path = c_ptr_to_string(ptr);
            if is_relative_path(&path) {
                dirfd_and_path(dirfd, &path)
            } else {
                Ok(path)
            }
        }
    }
}