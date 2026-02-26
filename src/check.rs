    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        // Test if PMULL 64-bit polynomial multiplication exists
        let a: poly64_t = 1;
        let b: poly64_t = !0;
        let _c = vmull_p64(a, b);
    }
