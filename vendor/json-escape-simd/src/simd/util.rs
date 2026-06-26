use crate::{NEED_ESCAPED, QUOTE_TAB};

#[inline(always)]
pub(crate) unsafe fn escape_unchecked(src: &mut *const u8, nb: &mut usize, dst: &mut *mut u8) {
    debug_assert!(*nb >= 1);
    loop {
        let ch = unsafe { *(*src) };
        let cnt = QUOTE_TAB[ch as usize].0 as usize;
        debug_assert!(
            cnt != 0,
            "char is {}, cnt is {},  NEED_ESCAPED is {}",
            ch as char,
            cnt,
            NEED_ESCAPED[ch as usize]
        );
        unsafe { std::ptr::copy_nonoverlapping(QUOTE_TAB[ch as usize].1.as_ptr(), *dst, 8) };
        unsafe { (*dst) = (*dst).add(cnt) };
        unsafe { (*src) = (*src).add(1) };
        (*nb) -= 1;
        if (*nb) == 0 || unsafe { NEED_ESCAPED[*(*src) as usize] == 0 } {
            return;
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[inline(always)]
pub(crate) fn check_cross_page(ptr: *const u8, step: usize) -> bool {
    let page_size = 4096;
    ((ptr as usize & (page_size - 1)) + step) > page_size
}
