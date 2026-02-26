#![feature(stdarch_aarch64_sve)]
#[cfg(target_arch = "aarch64")]
pub unsafe fn check_sve2() {
    use core::arch::asm;
    
    // Testing SVE2 svmatch emission
    // MATCH P0.B, P1/Z, Z0.B, Z1.B
    let input = [b'A'; 64];
    let mut out: u64;
    asm!(
        "match p0.b, p1/z, z0.b, z1.b",
        out = out(reg) out,
    );
}
