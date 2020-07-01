#![cfg_attr(not(test), no_std)]
#![cfg_attr(feature = "asm", feature(asm))]

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

    /// Extract the entire mantissa
    pub fn mantissa(self) -> u64 {
        self.range(0, 64) as u64 // 64-0 = 64 => always fits 64 bits
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
        #[cfg(all(feature = "asm", target_arch = "x86_64"))]
        { self.x86_f80_to_f64() }

        #[cfg(not(all(feature = "asm", target_arch = "x86_64")))]
        { self.emulate_f80_to_f64() }
    }
}
impl fmt::Debug for f80 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "f80({sign:+} * {int:01b}.{frac:063b} * 2^({exp:+}))", sign = if self.sign() { -1 } else { 1 }, exp = self.exp(), int = self.int() as u8, frac = self.fraction())
    }
}

impl f80 {
    #[cfg(all(feature = "asm", target_arch = "x86_64"))]
    fn x86_f80_to_f64(self) -> f64 {
        let mut float = 0f64;
        unsafe {
            asm!("
                  fld TBYTE PTR [{}]
                  fstp QWORD PTR [{}]
                ", in(reg) (&self.bits), in(reg) (&mut float));
        }
        float
    }
    #[allow(dead_code)]
    fn emulate_f80_to_f64(self) -> f64 {
        // Handle special cases
        if self.exp_bits() == 0x7FFF {
            match self.mantissa() >> (64 - 2) {
                0b00 | 0b10 => return if self.mantissa() == 0 {
                    f64::INFINITY
                } else {
                    f64::NAN
                },
                0b01 | 0b11 => return f64::NAN,
                _ => unreachable!("all 2-bit cases should be handled"),
            }
        }

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

#[cfg(test)]
mod tests {
    use super::f80;

    const EPSILON: f64 = 0.000001;

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
        let sqrt64 = f80::from_bits(302277571763841567555584).emulate_f80_to_f64();
        println!("sqrt(64) = {}", sqrt64);
        assert!((sqrt64 - 8.0).abs() < EPSILON);

        // sqrt(32)
        let sqrt32 = f80::from_bits(302262945465556336010372).emulate_f80_to_f64();
        println!("sqrt(32) = {}", sqrt32);
        assert!((sqrt32 - 5.65685424949238).abs() < EPSILON);
    }

    proptest::proptest! {
        #[test]
        #[cfg(all(target_arch = "x86_64", feature = "asm"))]
        fn emulated_works(n in 0..=u128::MAX) {
            let f = f80::from_bits(n);
            let expected = f.x86_f80_to_f64();
            let actual = f.emulate_f80_to_f64();
            println!("---");
            println!("expected: {}", expected);
            println!("actual: {}", actual);
            proptest::prop_assert!(actual - expected < EPSILON);
        }
    }
}
