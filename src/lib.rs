#![cfg_attr(test, feature(asm))]
// #![cfg_attr(not(test), no_std)]

use core::fmt;

/// An 80-bit float, internally stored using one 128-bit integer. This lets you
/// convert it back and forth from f64, and extract various parts of the type.
#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct f80 {
    bits: u128,
}
impl f80 {
    /// New f80 from specified bits. Will clear any bits over 80.
    pub fn from_bits(bits: u128) -> Self {
        Self {
            bits: bits & ((1 << 80) - 1),
        }
    }
    /// Convert to bits. The bits 80..128 will all be zero.
    pub fn to_bits(self) -> u128 {
        self.bits
    }

    /// Extract a specified (uninclusive) range of the bits.
    ///
    /// ```rust,ignore
    /// assert_eq!(f80::from_bits(0b00101010).range(0, 8), 42); // all bits
    /// assert_eq!(f80::from_bits(0b00100101).range(5, 7), 0b01); // only 2 bits
    /// ```
    ///
    ///   -- bits [5, 6] (5..7)
    /// 0 01 00101
    /// ^ --     ^
    /// └ bit 7  └ bit 0
    fn range(self, start: u8, end: u8) -> u128 {
        let mask = (1 << end) - 1; // 1111111... up to `end - 1` ones
        (self.bits & mask) >> start
    }

    /// Extract the fraction part of the mantissa
    pub fn fraction(self) -> u64 {
        self.range(0, 63) as u64 // 63-0 = 63 => always fits 64 bits
    }
    /// Extract the integer part of the mantissa
    pub fn int(self) -> bool {
        self.range(63, 64) == 1
    }
    /// Extract exponent part
    pub fn exp_bits(self) -> u16 {
        self.range(64, 79) as u16 // 79-64 = 15 => always fits 16 bits
    }
    /// Extract exponent part
    pub fn exp(self) -> i16 {
        self.exp_bits() as i16 - ((1 << 14) - 1)
    }
    /// Extract sign part
    pub fn sign(self) -> bool {
        self.range(79, 80) == 1
    }

    pub fn to_f64(self) -> f64 {
        // Truncate fraction
        let mut fraction = self.fraction() as u64;
        fraction >>= 64 - 53;

        // Convert f80 bias to f64 bias in exponent
        let mut exp = self.exp() as u64;
        let f64_bias = (1 << 10) - 1; // mentioned as 1023 in Wikipedia
        exp += f64_bias;

        // Get sign
        let sign = self.sign() as u64;

        // --- All parts done, assemble f64 ---

        let mut output = 0;

        // Push sign
        output |= sign;

        // Push exponent
        output <<= 11;
        output |= exp & ((1 << 11) - 1);

        // Push fraction. The explicit integer part of f80 is ignored, because
        // the f64 fraction implies there's an integer part of 1.
        output <<= 52;
        output |= fraction & ((1 << 52) - 1);

        f64::from_bits(output)
    }
}
impl fmt::Debug for f80 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "f80({sign:+} * {int:01b}.{frac:063b} * 2^({exp:+}))", sign = if self.sign() { -1 } else { 1 }, exp = self.exp(), int = self.int() as u8, frac = self.fraction())
    }
}


#[cfg(test)]
mod tests {
    use super::f80;

    #[test]
    fn range() {
        // Example in doctest
        assert_eq!(f80::from_bits(0b00101010).range(0, 8), 42); // all bits
        assert_eq!(f80::from_bits(0b00100101).range(5, 7), 0b01); // only 2 bits
    }

    #[test]
    fn extract() {
        let eight = f80::from_bits(0b0_100000000000010_1_000000000000000000000000000000000000000000000000000000000000000);

        assert_eq!(eight.sign(), false);
        assert_eq!(eight.exp_bits(), 0b100000000000010);
        assert_eq!(eight.exp(), 0b000000000000011);
        assert_eq!(eight.int(), true);
        assert_eq!(eight.fraction(), 0b000000000000000000000000000000000000000000000000000000000000000);
    }

    #[test]
    fn hardcoded_examples() {
        // sqrt(64)
        let sqrt64 = f80::from_bits(302277571763841567555584).to_f64();
        println!("sqrt(64) = {}", sqrt64);
        assert!((sqrt64 - 8.0).abs() < std::f64::EPSILON);

        // sqrt(32)
        let sqrt32 = f80::from_bits(302262945465556336010372).to_f64();
        println!("sqrt(32) = {}", sqrt32);
        assert!((sqrt32 - 5.65685424949238).abs() < std::f64::EPSILON);
    }

    #[cfg(target_arch = "x86_64")]
    fn x86_64_divide(n: u64, d: u64) -> bool {
        #[derive(Clone, Copy, Debug, Default)]
        #[repr(packed)]
        pub struct FloatRegisters {
            pub fcw: u16,
            pub fsw: u16,
            pub ftw: u8,
            pub _reserved0: u8,
            pub fop: u16,
            pub fip: u64,
            pub fdp: u64,
            pub mxcsr: u32,
            pub mxcsr_mask: u32,
            pub st_space: [u128; 8],
            pub xmm_space: [u128; 16],
            pub _reserved1: [u128; 3],
            pub _available0: [u128; 3],
        }

        #[repr(align(16))]
        pub struct Aligned(pub FloatRegisters);

        let mut fx = Aligned(FloatRegisters::default());
        let mut res: u64 = 0;

        unsafe {
            asm!("fild QWORD PTR [{}] // push n to float stack
                  fild QWORD PTR [{}] // push d to float stack
                  fdivp               // divide
                  fxsave64 [{}]       // store floating point data
                  fistp QWORD PTR [{}]
                  ", in(reg) (&n), in(reg) (&d), in(reg) (&mut fx), in(reg) (&mut res));
        }

        let out = f80::from_bits(fx.0.st_space[fx.0.fop as usize]);

        println!("{} / {} = {:?}", n, d, out);
        println!("float value: {}", out.to_f64());

        ((n as f64 / d as f64) - out.to_f64()).abs() < 0.000001
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn x86_64_divide_hardcoded() {
        assert!(x86_64_divide(100, 1));
        assert!(x86_64_divide(100, 2));

        for _ in 0..100 {
            assert!(x86_64_divide(184, 1));
        }
    }

    proptest::proptest! {
        #[test]
        #[cfg(target_arch = "x86_64")]
        fn x86_64_divide_prop(n in 100u64..1000u64, d in 1u64..100u64) {
            proptest::prop_assert!(x86_64_divide(n, d));
        }
    }
}
