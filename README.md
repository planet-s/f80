# f80

The `f80` library is a simple Rust interface to access 80-bit floats by
converting them to f64. The conversion is not lossless, of course, but it's as
close as it can get since `f80` uses architecture-specific machine code to do
the conversion for you.

You can also extract the various components from the floats, like the fraction
part, the exponent, etc. In the future, we might provide a fallback conversion
method using these parts that doesn't rely on machine code.

## Motivation

This crate was mainly done to aid [our custom
gdbserver](https://gitlab.redox-os.org/redox-os/gdbserver) in sending
floating-point values directly from the `ST(i)` registers, to GDB.
