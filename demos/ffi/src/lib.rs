use std::ffi::{CString, c_char};

#[unsafe(no_mangle)]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    println!("* FFI | ADD: {a} + {b}");
    a + b
}

#[unsafe(no_mangle)]
pub extern "C" fn ensure_42(v: i32) {
    println!("* FFI | ENSURE 42: {v}");
    assert_eq!(v, 42);
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn print_string(v: *mut c_char) {
    let v = unsafe { CString::from_raw(v) };
    println!("* FFI | PRINT STRING: {:?}", v.as_c_str().to_str().unwrap());
}
