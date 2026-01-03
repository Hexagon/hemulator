//! MI (MIPS Interface) - Interrupt controller for Nintendo 64
//!
//! The MI is responsible for:
//! - Managing interrupts from various hardware components (VI, SI, PI, AI, DP, SP)
//! - Providing interrupt masking
//! - Controlling system mode settings
//!
//! ## Memory Map
//!
//! MI registers are memory-mapped at 0x04300000-0x0430000F:
//! - 0x04300000: MI_MODE - System mode control
//! - 0x04300004: MI_VERSION - Hardware version
//! - 0x04300008: MI_INTR - Interrupt status (read-only)
//! - 0x0430000C: MI_INTR_MASK - Interrupt mask (read/write)
//!
//! ## Interrupt Bits
//!
//! Both MI_INTR and MI_INTR_MASK use the same bit positions:
//! - Bit 0: SP (Signal Processor)
//! - Bit 1: SI (Serial Interface)
//! - Bit 2: AI (Audio Interface)
//! - Bit 3: VI (Video Interface)
//! - Bit 4: PI (Peripheral Interface)
//! - Bit 5: DP (Display Processor)

/// MI register offsets (relative to 0x04300000)
const MI_MODE: u32 = 0x00;
const MI_VERSION: u32 = 0x04;
const MI_INTR: u32 = 0x08;
const MI_INTR_MASK: u32 = 0x0C;

/// Interrupt bit positions
pub const MI_INTR_SP: u32 = 0x01; // Bit 0
#[allow(dead_code)]
const MI_INTR_SI: u32 = 0x02; // Bit 1
#[allow(dead_code)]
const MI_INTR_AI: u32 = 0x04; // Bit 2
pub const MI_INTR_VI: u32 = 0x08; // Bit 3
#[allow(dead_code)]
const MI_INTR_PI: u32 = 0x10; // Bit 4
pub const MI_INTR_DP: u32 = 0x20; // Bit 5

/// MIPS Interface (MI) - Interrupt controller
pub struct MipsInterface {
    /// MI_MODE register - system mode control
    mode: u32,
    /// MI_VERSION register - hardware version (read-only)
    version: u32,
    /// MI_INTR register - interrupt status (read-only, set by hardware)
    intr: u32,
    /// MI_INTR_MASK register - interrupt mask
    intr_mask: u32,
}

impl MipsInterface {
    /// Create a new MIPS Interface
    pub fn new() -> Self {
        Self {
            mode: 0,
            version: 0x02020102, // N64 hardware version
            intr: 0,
            intr_mask: 0,
        }
    }

    /// Reset to initial state
    #[allow(dead_code)] // Reserved for future use
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read from MI register
    pub fn read_register(&self, offset: u32) -> u32 {
        match offset {
            MI_MODE => self.mode,
            MI_VERSION => self.version,
            MI_INTR => self.intr,
            MI_INTR_MASK => self.intr_mask,
            _ => 0,
        }
    }

    /// Write to MI register
    pub fn write_register(&mut self, offset: u32, value: u32) {
        match offset {
            MI_MODE => {
                // MI_MODE has special write behavior:
                // Bit 7: clear init mode (bit 0)
                // Bit 8: set init mode (bit 0)
                // Bit 9: clear ebus test mode (bit 1)
                // Bit 10: set ebus test mode (bit 1)
                // Bit 11: clear DP interrupt (bit 2)
                // Bit 12: clear RDRAM reg mode (bit 3)
                // Bit 13: set RDRAM reg mode (bit 3)

                if value & (1 << 7) != 0 {
                    self.mode &= !(1 << 0); // Clear init mode
                }
                if value & (1 << 8) != 0 {
                    self.mode |= 1 << 0; // Set init mode
                }
                if value & (1 << 9) != 0 {
                    self.mode &= !(1 << 1); // Clear ebus test mode
                }
                if value & (1 << 10) != 0 {
                    self.mode |= 1 << 1; // Set ebus test mode
                }
                if value & (1 << 11) != 0 {
                    self.intr &= !(1 << 5); // Clear DP interrupt
                }
                if value & (1 << 12) != 0 {
                    self.mode &= !(1 << 3); // Clear RDRAM reg mode
                }
                if value & (1 << 13) != 0 {
                    self.mode |= 1 << 3; // Set RDRAM reg mode
                }
            }
            MI_VERSION => {
                // MI_VERSION is read-only, ignore writes
            }
            MI_INTR => {
                // MI_INTR is read-only (status register), but writes clear bits
                self.intr &= !value;
            }
            MI_INTR_MASK => {
                // MI_INTR_MASK has special write behavior for setting/clearing bits:
                // Bits 0-5: clear corresponding mask bit
                // Bits 8-13: set corresponding mask bit

                // Clear mask bits (bits 0-5)
                self.intr_mask &= !(value & 0x3F);

                // Set mask bits (bits 8-13 correspond to mask bits 0-5)
                let set_bits = (value >> 8) & 0x3F;
                self.intr_mask |= set_bits;
            }
            _ => {}
        }
    }

    /// Set an interrupt bit (called by hardware components)
    pub fn set_interrupt(&mut self, interrupt_bit: u32) {
        self.intr |= interrupt_bit;
    }

    /// Clear an interrupt bit (called by hardware components)
    #[allow(dead_code)] // Reserved for future use
    pub fn clear_interrupt(&mut self, interrupt_bit: u32) {
        self.intr &= !interrupt_bit;
    }

    /// Check if any interrupts are pending and unmasked
    #[allow(dead_code)] // Used in tests
    pub fn has_pending_interrupt(&self) -> bool {
        (self.intr & self.intr_mask) != 0
    }

    /// Get the current interrupt status (masked)
    #[allow(dead_code)] // Used in tests
    pub fn get_pending_interrupts(&self) -> u32 {
        self.intr & self.intr_mask
    }

    /// Get the raw interrupt status (unmasked)
    pub fn get_interrupt_status(&self) -> u32 {
        self.intr
    }
}

impl Default for MipsInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mi_creation() {
        let mi = MipsInterface::new();
        assert_eq!(mi.read_register(MI_MODE), 0);
        assert_eq!(mi.read_register(MI_VERSION), 0x02020102);
        assert_eq!(mi.read_register(MI_INTR), 0);
        assert_eq!(mi.read_register(MI_INTR_MASK), 0);
    }

    #[test]
    fn test_mi_reset() {
        let mut mi = MipsInterface::new();
        mi.set_interrupt(MI_INTR_VI);
        mi.write_register(MI_INTR_MASK, 0x0800); // Enable VI interrupt

        mi.reset();

        assert_eq!(mi.read_register(MI_INTR), 0);
        assert_eq!(mi.read_register(MI_INTR_MASK), 0);
    }

    #[test]
    fn test_mi_set_interrupt() {
        let mut mi = MipsInterface::new();

        mi.set_interrupt(MI_INTR_VI);
        assert_eq!(mi.read_register(MI_INTR), MI_INTR_VI);

        mi.set_interrupt(MI_INTR_SP);
        assert_eq!(mi.read_register(MI_INTR), MI_INTR_VI | MI_INTR_SP);
    }

    #[test]
    fn test_mi_clear_interrupt() {
        let mut mi = MipsInterface::new();

        mi.set_interrupt(MI_INTR_VI | MI_INTR_SP);
        assert_eq!(mi.read_register(MI_INTR), MI_INTR_VI | MI_INTR_SP);

        mi.clear_interrupt(MI_INTR_VI);
        assert_eq!(mi.read_register(MI_INTR), MI_INTR_SP);
    }

    #[test]
    fn test_mi_intr_write_clears() {
        let mut mi = MipsInterface::new();

        mi.set_interrupt(MI_INTR_VI | MI_INTR_SP);
        assert_eq!(mi.read_register(MI_INTR), MI_INTR_VI | MI_INTR_SP);

        // Writing to MI_INTR clears the specified bits
        mi.write_register(MI_INTR, MI_INTR_VI);
        assert_eq!(mi.read_register(MI_INTR), MI_INTR_SP);
    }

    #[test]
    fn test_mi_intr_mask_write() {
        let mut mi = MipsInterface::new();

        // Set VI interrupt mask (bit 8 in write corresponds to mask bit 3)
        mi.write_register(MI_INTR_MASK, 0x0800);
        assert_eq!(mi.read_register(MI_INTR_MASK), 0x08);

        // Set SP interrupt mask (bit 8 in write corresponds to mask bit 0)
        mi.write_register(MI_INTR_MASK, 0x0100);
        assert_eq!(mi.read_register(MI_INTR_MASK), 0x09); // Both VI and SP

        // Clear VI interrupt mask (bit 3)
        mi.write_register(MI_INTR_MASK, 0x08);
        assert_eq!(mi.read_register(MI_INTR_MASK), 0x01); // Only SP
    }

    #[test]
    fn test_mi_has_pending_interrupt() {
        let mut mi = MipsInterface::new();

        // No interrupt pending
        assert!(!mi.has_pending_interrupt());

        // Set interrupt but no mask
        mi.set_interrupt(MI_INTR_VI);
        assert!(!mi.has_pending_interrupt());

        // Enable mask
        mi.write_register(MI_INTR_MASK, 0x0800);
        assert!(mi.has_pending_interrupt());

        // Clear interrupt
        mi.write_register(MI_INTR, MI_INTR_VI);
        assert!(!mi.has_pending_interrupt());
    }

    #[test]
    fn test_mi_get_pending_interrupts() {
        let mut mi = MipsInterface::new();

        // Set multiple interrupts
        mi.set_interrupt(MI_INTR_VI | MI_INTR_SP | MI_INTR_AI);

        // Enable only VI and SP masks
        mi.write_register(MI_INTR_MASK, 0x0900); // Bits 8 and 11

        // Should only get VI and SP as pending (masked)
        let pending = mi.get_pending_interrupts();
        assert_eq!(pending, MI_INTR_VI | MI_INTR_SP);
    }

    #[test]
    fn test_mi_mode_register() {
        let mut mi = MipsInterface::new();

        // Set init mode (bit 8 in write)
        mi.write_register(MI_MODE, 1 << 8);
        assert_eq!(mi.read_register(MI_MODE) & 0x01, 0x01);

        // Clear init mode (bit 7 in write)
        mi.write_register(MI_MODE, 1 << 7);
        assert_eq!(mi.read_register(MI_MODE) & 0x01, 0x00);
    }

    #[test]
    fn test_mi_version_readonly() {
        let mut mi = MipsInterface::new();
        let version = mi.read_register(MI_VERSION);

        // Try to write to version register
        mi.write_register(MI_VERSION, 0xDEADBEEF);

        // Should still have original value
        assert_eq!(mi.read_register(MI_VERSION), version);
    }
}
