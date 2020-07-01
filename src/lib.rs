#![cfg_attr(not(test), no_std)]
#![feature(asm)]

use core::fmt;

#[cfg(target_arch = "x86_64")]
#[path = "arch/x86_64.rs"]
mod sys;

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

    /// Convert f64 to f80
    pub fn from_f64(float: f64) -> Self {
        let mut bytes = [0; 16];
        let start = if cfg!(target_endian = "big") { 6 } else { 0 };

        unsafe {
            sys::load_f64_into_f80(&float, bytes[start..].as_mut_ptr());
        }

        Self {
            bits: u128::from_ne_bytes(bytes),
        }
    }

    /// Convert f80 to f64
    pub fn to_f64(self) -> f64 {
        let bytes = self.bits.to_ne_bytes();
        let start = if cfg!(target_endian = "big") { 6 } else { 0 };

        let mut float = 0f64;
        unsafe {
            sys::load_f80_into_f64(bytes[start..].as_ptr(), &mut float);
        }

        float
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
    fn to_f64() {
        // sqrt(64)
        let sqrt64 = f80::from_bits(302277571763841567555584).to_f64();
        println!("sqrt(64) = {}", sqrt64);
        assert!((sqrt64 - 8.0).abs() < EPSILON);

        // sqrt(32)
        let sqrt32 = f80::from_bits(302262945465556336010372).to_f64();
        println!("sqrt(32) = {}", sqrt32);
        assert!((sqrt32 - 5.65685424949238).abs() < EPSILON);
    }

    #[test]
    fn from_f64() {
        assert_eq!(f80::from_f64(8.0).to_bits(), 302277571763841567555584);
    }
}
