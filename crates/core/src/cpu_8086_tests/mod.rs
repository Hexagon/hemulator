//! Tests for Intel 8086 CPU implementation
//!
//! Tests are organized by operation size and type:
//! - `tests_8bit`: 8-bit ALU operations and data movement
//! - `tests_16bit`: 16-bit ALU operations and data movement
//! - `tests_32bit`: 32-bit operations (80386+)
//! - `tests_jumps`: Jump, call, return, and loop instructions
//! - `tests_flags`: Flag manipulation and testing
//! - `tests_misc`: System instructions, I/O, MMX, and special operations

mod tests_8bit;
mod tests_16bit;
mod tests_32bit;
mod tests_jumps;
mod tests_flags;
mod tests_misc;
