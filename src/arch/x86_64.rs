pub unsafe fn load_f80_into_f64(bits: *const u8, out: *mut f64) {
    asm!("
      fld TBYTE PTR [{}]
      fstp QWORD PTR [{}]
    ", in(reg) bits, in(reg) out);
}
