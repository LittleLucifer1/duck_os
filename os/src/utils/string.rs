use alloc::string::String;


pub fn c_ptr_to_string(ptr: *const u8) -> String {
    let mut ptr_usize=  ptr as usize;
    let mut name = String::new();
    loop {
        let ch = unsafe {*(ptr_usize as *const u8)};
        if ch == 0 {
            break;
        } 
        name.push(ch as char);
        ptr_usize += 1;
    }
    name
}