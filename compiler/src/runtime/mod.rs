//! Sengoo 运行时
//!
//! 提供基础的运行时支持功能。

// ============================================================================
// 内存管理
// ============================================================================

/// 内存分配器
pub struct Allocator {
    _private: [u8; 0],
}

impl Allocator {
    /// 分配内存
    #[no_mangle]
    pub extern "C" fn sengoo_alloc(size: usize, mut align: usize) -> *mut u8 {
        if align == 0 {
            align = 1;
        }
        let layout = std::alloc::Layout::from_size_align(size, align).unwrap();
        unsafe { std::alloc::alloc(layout) }
    }

    /// 释放内存
    #[no_mangle]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn sengoo_free(ptr: *mut u8, size: usize, align: usize) {
        if ptr.is_null() {
            return;
        }
        if align == 0 {
            return;
        }
        let layout = std::alloc::Layout::from_size_align(size, align).unwrap();
        unsafe { std::alloc::dealloc(ptr, layout) };
    }

    /// 重新分配内存
    #[no_mangle]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn sengoo_realloc(
        ptr: *mut u8,
        old_size: usize,
        old_align: usize,
        new_size: usize,
    ) -> *mut u8 {
        if old_align == 0 {
            return std::ptr::null_mut();
        }
        let old_layout = std::alloc::Layout::from_size_align(old_size, old_align).unwrap();
        unsafe { std::alloc::realloc(ptr, old_layout, new_size) }
    }
}

// ============================================================================
// 字符串处理
// ============================================================================

/// 字符串长度
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_str_len(s: *const u8) -> usize {
    if s.is_null() {
        return 0;
    }
    unsafe {
        let mut len = 0;
        let mut ptr = s;
        while *ptr != 0 {
            len += 1;
            ptr = ptr.add(1);
        }
        len
    }
}

/// Return the raw pointer value backing a Sengoo `&str`.
#[no_mangle]
pub extern "C" fn sengoo_stdlib_str_ptr(s: *const u8) -> i64 {
    s as isize as i64
}

/// 字符串比较
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_str_compare(s1: *const u8, s2: *const u8) -> i32 {
    if s1.is_null() && s2.is_null() {
        return 0;
    }
    if s1.is_null() {
        return -1;
    }
    if s2.is_null() {
        return 1;
    }
    unsafe {
        let mut p1 = s1;
        let mut p2 = s2;
        loop {
            let c1 = *p1;
            let c2 = *p2;
            if c1 < c2 {
                return -1;
            }
            if c1 > c2 {
                return 1;
            }
            if c1 == 0 {
                return 0;
            }
            p1 = p1.add(1);
            p2 = p2.add(1);
        }
    }
}

/// 字符串复制
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_str_copy(src: *const u8, dst: *mut u8, max_len: usize) -> usize {
    if src.is_null() || dst.is_null() {
        return 0;
    }
    unsafe {
        let mut copied = 0;
        let mut s = src;
        let mut d = dst;
        while copied < max_len {
            let c = *s;
            *d = c;
            if c == 0 {
                break;
            }
            copied += 1;
            s = s.add(1);
            d = d.add(1);
        }
        // 确保以 null 结尾
        if copied < max_len {
            *d = 0;
        }
        copied
    }
}

/// 字符串拼接 - 分配新字符串
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_str_concat(s1: *const u8, s2: *const u8) -> *mut u8 {
    let len1 = sengoo_str_len(s1);
    let len2 = sengoo_str_len(s2);
    let total = len1 + len2 + 1; // +1 for null terminator

    let result = Allocator::sengoo_alloc(total, 1);
    if result.is_null() {
        return std::ptr::null_mut();
    }
    if !s1.is_null() {
        sengoo_memcpy(result, s1, len1);
    }
    if !s2.is_null() {
        unsafe {
            sengoo_memcpy(result.add(len1), s2, len2);
        }
    }
    unsafe {
        *result.add(len1 + len2) = 0; // null terminate
    }
    result
}

/// 字符串相等比较
#[no_mangle]
pub extern "C" fn sengoo_str_eq(s1: *const u8, s2: *const u8) -> i64 {
    if sengoo_str_compare(s1, s2) == 0 {
        1
    } else {
        0
    }
}

// ============================================================================
// 打印输出
// ============================================================================

/// 打印 i64 值到 stdout
#[no_mangle]
pub extern "C" fn sengoo_print_i64(value: i64) {
    println!("{}", value);
}

/// 打印 bool 值到 stdout
#[no_mangle]
pub extern "C" fn sengoo_print_bool(value: i64) {
    // LLVM i1 会被 zext 到 i64
    println!("{}", if value != 0 { "true" } else { "false" });
}

/// 打印 f64 值到 stdout
#[no_mangle]
pub extern "C" fn sengoo_print_f64(value: f64) {
    println!("{}", value);
}

/// 打印字符串到 stdout
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_print_str(ptr: *const u8) {
    if ptr.is_null() {
        println!();
        return;
    }
    unsafe {
        let len = sengoo_str_len(ptr);
        let slice = std::slice::from_raw_parts(ptr, len);
        let s = String::from_utf8_lossy(slice);
        println!("{}", s);
    }
}

// ============================================================================
// 数学函数
// ============================================================================

/// 幂运算
#[no_mangle]
pub extern "C" fn sengoo_pow_f32(base: f32, exp: f32) -> f32 {
    base.powf(exp)
}

/// 幂运算
#[no_mangle]
pub extern "C" fn sengoo_pow_f64(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

/// 平方根
#[no_mangle]
pub extern "C" fn sengoo_sqrt_f32(x: f32) -> f32 {
    x.sqrt()
}

/// 平方根
#[no_mangle]
pub extern "C" fn sengoo_sqrt_f64(x: f64) -> f64 {
    x.sqrt()
}

/// 绝对值
#[no_mangle]
pub extern "C" fn sengoo_abs_i32(x: i32) -> i32 {
    x.abs()
}

/// 绝对值
#[no_mangle]
pub extern "C" fn sengoo_abs_i64(x: i64) -> i64 {
    x.abs()
}

/// 绝对值
#[no_mangle]
pub extern "C" fn sengoo_abs_f32(x: f32) -> f32 {
    x.abs()
}

/// 绝对值
#[no_mangle]
pub extern "C" fn sengoo_abs_f64(x: f64) -> f64 {
    x.abs()
}

// ============================================================================
// 检查算术
// ============================================================================

/// 带溢出检查的加法
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_add_overflow_i32(a: i32, b: i32, overflow: *mut bool) -> i32 {
    match a.checked_add(b) {
        Some(result) => {
            unsafe {
                *overflow = false;
            }
            result
        }
        None => {
            unsafe {
                *overflow = true;
            }
            a.wrapping_add(b)
        }
    }
}

/// 带溢出检查的减法
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_sub_overflow_i32(a: i32, b: i32, overflow: *mut bool) -> i32 {
    match a.checked_sub(b) {
        Some(result) => {
            unsafe {
                *overflow = false;
            }
            result
        }
        None => {
            unsafe {
                *overflow = true;
            }
            a.wrapping_sub(b)
        }
    }
}

/// 带溢出检查的乘法
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_mul_overflow_i32(a: i32, b: i32, overflow: *mut bool) -> i32 {
    match a.checked_mul(b) {
        Some(result) => {
            unsafe {
                *overflow = false;
            }
            result
        }
        None => {
            unsafe {
                *overflow = true;
            }
            a.wrapping_mul(b)
        }
    }
}

// ============================================================================
// 内存操作
// ============================================================================

/// 内存复制
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_memcpy(dst: *mut u8, src: *const u8, count: usize) {
    if dst.is_null() || src.is_null() || count == 0 {
        return;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(src, dst, count);
    }
}

/// 内存移动
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_memmove(dst: *mut u8, src: *const u8, count: usize) {
    if dst.is_null() || src.is_null() || count == 0 {
        return;
    }
    unsafe {
        std::ptr::copy(src, dst, count);
    }
}

/// 内存填充
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_memset(dst: *mut u8, value: u8, count: usize) {
    if dst.is_null() || count == 0 {
        return;
    }
    unsafe {
        std::ptr::write_bytes(dst, value, count);
    }
}

/// 内存比较
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_memcmp(s1: *const u8, s2: *const u8, count: usize) -> i32 {
    if s1.is_null() && s2.is_null() {
        return 0;
    }
    if s1.is_null() {
        return -1;
    }
    if s2.is_null() {
        return 1;
    }
    unsafe {
        for i in 0..count {
            let c1 = *s1.add(i);
            let c2 = *s2.add(i);
            if c1 < c2 {
                return -1;
            }
            if c1 > c2 {
                return 1;
            }
        }
        0
    }
}

// ============================================================================
// panic 处理
// ============================================================================

/// Panic 处理函数
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_panic(msg: *const u8, len: usize) -> ! {
    if !msg.is_null() && len > 0 {
        let slice = unsafe { std::slice::from_raw_parts(msg, len) };
        let s = String::from_utf8_lossy(slice);
        eprintln!("Sengoo panic: {}", s);
    } else {
        eprintln!("Sengoo panic: <no message>");
    }
    std::process::exit(1);
}

/// 断言失败
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn sengoo_assert_fail(
    file: *const u8,
    file_len: usize,
    line: u32,
    msg: *const u8,
    msg_len: usize,
) -> ! {
    eprint!("Assertion failed: ");
    if !msg.is_null() && msg_len > 0 {
        let slice = unsafe { std::slice::from_raw_parts(msg, msg_len) };
        let s = String::from_utf8_lossy(slice);
        eprint!("{}", s);
    }
    eprint!(" at ");
    if !file.is_null() && file_len > 0 {
        let slice = unsafe { std::slice::from_raw_parts(file, file_len) };
        let s = String::from_utf8_lossy(slice);
        eprint!("{}:{}", s, line);
    } else {
        eprint!("unknown:{}", line);
    }
    eprintln!();
    std::process::exit(1);
}

// ============================================================================
// 测试和导出
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_str_len() {
        let s = b"hello\0";
        assert_eq!(sengoo_str_len(s.as_ptr()), 5);
    }

    #[test]
    fn test_str_compare() {
        let s1 = b"hello\0";
        let s2 = b"hello\0";
        assert_eq!(sengoo_str_compare(s1.as_ptr(), s2.as_ptr()), 0);

        let s3 = b"world\0";
        assert!(sengoo_str_compare(s1.as_ptr(), s3.as_ptr()) < 0);
    }

    #[test]
    fn test_abs() {
        assert_eq!(sengoo_abs_i32(-42), 42);
        assert_eq!(sengoo_abs_i32(42), 42);
        assert_eq!(sengoo_abs_f64(-std::f64::consts::PI), std::f64::consts::PI);
    }

    #[test]
    fn test_overflow() {
        let mut overflow = false;
        let _result = sengoo_add_overflow_i32(2000000000, 2000000000, &mut overflow);
        assert!(overflow);

        overflow = false;
        let result = sengoo_add_overflow_i32(1, 2, &mut overflow);
        assert!(!overflow);
        assert_eq!(result, 3);
    }

    #[test]
    fn test_print_i64() {
        // Should not panic for various values
        sengoo_print_i64(0);
        sengoo_print_i64(42);
        sengoo_print_i64(-1);
        sengoo_print_i64(i64::MAX);
        sengoo_print_i64(i64::MIN);
    }

    #[test]
    fn test_print_bool() {
        // value != 0 → "true", value == 0 → "false"
        sengoo_print_bool(0); // prints "false"
        sengoo_print_bool(1); // prints "true"
        sengoo_print_bool(-1); // prints "true" (non-zero)
    }

    #[test]
    fn test_print_f64() {
        sengoo_print_f64(0.0);
        sengoo_print_f64(std::f64::consts::PI);
        sengoo_print_f64(-std::f64::consts::E);
        sengoo_print_f64(f64::INFINITY);
        sengoo_print_f64(f64::NAN);
    }

    #[test]
    fn test_print_str() {
        let s = b"hello\0";
        sengoo_print_str(s.as_ptr());
    }

    #[test]
    fn test_print_str_empty() {
        let s = b"\0";
        sengoo_print_str(s.as_ptr());
    }

    #[test]
    fn test_print_str_null() {
        // Should handle null pointer gracefully without crashing
        sengoo_print_str(std::ptr::null());
    }

    #[test]
    fn test_str_concat_basic() {
        let s1 = b"hello\0";
        let s2 = b" world\0";
        let result = sengoo_str_concat(s1.as_ptr(), s2.as_ptr());
        assert!(!result.is_null());
        let len = sengoo_str_len(result);
        assert_eq!(len, 11); // "hello world"
        let slice = unsafe { std::slice::from_raw_parts(result, len) };
        assert_eq!(slice, b"hello world");
    }

    #[test]
    fn test_str_concat_empty_strings() {
        let s1 = b"\0";
        let s2 = b"\0";
        let result = sengoo_str_concat(s1.as_ptr(), s2.as_ptr());
        assert!(!result.is_null());
        let len = sengoo_str_len(result);
        assert_eq!(len, 0);
    }

    #[test]
    fn test_str_concat_first_null() {
        let s2 = b"world\0";
        let result = sengoo_str_concat(std::ptr::null(), s2.as_ptr());
        assert!(!result.is_null());
        let len = sengoo_str_len(result);
        assert_eq!(len, 5);
        let slice = unsafe { std::slice::from_raw_parts(result, len) };
        assert_eq!(slice, b"world");
    }

    #[test]
    fn test_str_concat_second_null() {
        let s1 = b"hello\0";
        let result = sengoo_str_concat(s1.as_ptr(), std::ptr::null());
        assert!(!result.is_null());
        let len = sengoo_str_len(result);
        assert_eq!(len, 5);
        let slice = unsafe { std::slice::from_raw_parts(result, len) };
        assert_eq!(slice, b"hello");
    }

    #[test]
    fn test_str_concat_both_null() {
        let result = sengoo_str_concat(std::ptr::null(), std::ptr::null());
        assert!(!result.is_null());
        let len = sengoo_str_len(result);
        assert_eq!(len, 0);
    }

    #[test]
    fn test_str_eq_equal() {
        let s1 = b"hello\0";
        let s2 = b"hello\0";
        assert_eq!(sengoo_str_eq(s1.as_ptr(), s2.as_ptr()), 1);
    }

    #[test]
    fn test_str_eq_not_equal() {
        let s1 = b"hello\0";
        let s2 = b"world\0";
        assert_eq!(sengoo_str_eq(s1.as_ptr(), s2.as_ptr()), 0);
    }

    #[test]
    fn test_str_eq_both_null() {
        assert_eq!(sengoo_str_eq(std::ptr::null(), std::ptr::null()), 1);
    }

    #[test]
    fn test_str_eq_one_null() {
        let s1 = b"hello\0";
        assert_eq!(sengoo_str_eq(s1.as_ptr(), std::ptr::null()), 0);
        assert_eq!(sengoo_str_eq(std::ptr::null(), s1.as_ptr()), 0);
    }

    #[test]
    fn test_str_eq_empty_strings() {
        let s1 = b"\0";
        let s2 = b"\0";
        assert_eq!(sengoo_str_eq(s1.as_ptr(), s2.as_ptr()), 1);
    }
}
