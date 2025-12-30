//! PIF (Peripheral Interface) - Controller and boot ROM interface
//!
//! The PIF chip handles:
//! - Boot ROM execution (IPL3 bootstrap)
//! - Controller communication (N64 controllers, memory cards, etc.)
//! - EEPROM save data access
//! - RTC (Real-Time Clock) for some games
//!
//! # Controller Interface
//!
//! N64 controllers communicate via the PIF using a command/response protocol:
//! - **Command 0x00**: Controller info/status
//! - **Command 0x01**: Read controller state (buttons, stick)
//! - **Command 0x02**: Read controller pak (memory card)
//! - **Command 0x03**: Write controller pak
//!
//! Controller state is accessed via PIF RAM at address 0x1FC007C0-0x1FC007FF
//! Games write command blocks to PIF RAM, then read response blocks.
//!
//! ## Button State Convention
//!
//! **IMPORTANT**: N64 controllers use **active-high logic** for button states:
//! - **1 = Button pressed** (bit set)
//! - **0 = Button released** (bit clear)
//!
//! This is different from some other systems:
//! - Game Boy uses active-low (0 = pressed, 1 = released)
//! - NES uses active-high (1 = pressed, 0 = released)
//!
//! Button layout in 16-bit response:
//! - Bits 15-12: A, B, Z, Start
//! - Bits 11-8: D-Up, D-Down, D-Left, D-Right
//! - Bits 7-6: Reserved
//! - Bits 5-4: L, R
//! - Bits 3-0: C-Up, C-Down, C-Left, C-Right
//!
//! Analog stick uses signed 8-bit range:
//! - X axis: -128 (left) to +127 (right)
//! - Y axis: -128 (down) to +127 (up)
//!
//! # Implementation
//!
//! This is a simplified PIF implementation:
//! - Basic controller communication (buttons and analog stick)
//! - No memory card support (yet)
//! - No EEPROM support (yet)
//! - Minimal boot ROM (just enough to start games)

/// N64 controller button flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControllerButtons {
    /// A button
    pub a: bool,
    /// B button
    pub b: bool,
    /// Z trigger
    pub z: bool,
    /// Start button
    pub start: bool,
    /// D-pad Up
    pub d_up: bool,
    /// D-pad Down
    pub d_down: bool,
    /// D-pad Left
    pub d_left: bool,
    /// D-pad Right
    pub d_right: bool,
    /// L trigger
    pub l: bool,
    /// R trigger
    pub r: bool,
    /// C-Up button
    pub c_up: bool,
    /// C-Down button
    pub c_down: bool,
    /// C-Left button
    pub c_left: bool,
    /// C-Right button
    pub c_right: bool,
}

impl ControllerButtons {
    /// Pack buttons into 16-bit value for controller state response
    /// Bit layout (from MSB to LSB):
    /// 15: A, 14: B, 13: Z, 12: Start
    /// 11: D-Up, 10: D-Down, 9: D-Left, 8: D-Right
    /// 7: ?, 6: ?, 5: L, 4: R
    /// 3: C-Up, 2: C-Down, 1: C-Left, 0: C-Right
    pub fn to_u16(&self) -> u16 {
        let mut value = 0u16;

        if self.a {
            value |= 1 << 15;
        }
        if self.b {
            value |= 1 << 14;
        }
        if self.z {
            value |= 1 << 13;
        }
        if self.start {
            value |= 1 << 12;
        }
        if self.d_up {
            value |= 1 << 11;
        }
        if self.d_down {
            value |= 1 << 10;
        }
        if self.d_left {
            value |= 1 << 9;
        }
        if self.d_right {
            value |= 1 << 8;
        }
        if self.l {
            value |= 1 << 5;
        }
        if self.r {
            value |= 1 << 4;
        }
        if self.c_up {
            value |= 1 << 3;
        }
        if self.c_down {
            value |= 1 << 2;
        }
        if self.c_left {
            value |= 1 << 1;
        }
        if self.c_right {
            value |= 1 << 0;
        }

        value
    }
}

/// Controller state (buttons + analog stick)
#[derive(Debug, Clone, Copy, Default)]
pub struct ControllerState {
    /// Button states
    pub buttons: ControllerButtons,
    /// Analog stick X (-128 to 127, left to right)
    pub stick_x: i8,
    /// Analog stick Y (-128 to 127, down to up)
    pub stick_y: i8,
}

/// PIF (Peripheral Interface) state
pub struct Pif {
    /// PIF RAM (2KB)
    ram: [u8; 0x800],

    /// Controller 1 state
    controller1: ControllerState,

    /// Controller 2 state
    controller2: ControllerState,

    /// Controller 3 state
    controller3: ControllerState,

    /// Controller 4 state
    controller4: ControllerState,
}

impl Pif {
    /// Create new PIF with default state
    pub fn new() -> Self {
        Self {
            ram: [0; 0x800],
            controller1: ControllerState::default(),
            controller2: ControllerState::default(),
            controller3: ControllerState::default(),
            controller4: ControllerState::default(),
        }
    }

    /// Initialize PIF ROM in RAM
    pub fn init_rom(&mut self) {
        // PIF ROM starts at offset 0 in PIF RAM
        // Simplified boot: Jump to cartridge code at 0x10001000 (cached 0x90001000)

        let pif_rom: Vec<u32> = vec![
            // Jump to test ROM code at 0x10001000 (cartridge ROM + 0x1000)
            // Using cached address 0x90001000 (KSEG0 cached)
            0x3C089000, // lui $t0, 0x9000  # Upper 16 bits
            0x35081000, // ori $t0, $t0, 0x1000  # Lower 16 bits = 0x90001000
            0x01000008, // jr $t0  # Jump to $t0
            0x00000000, // nop (delay slot)
        ];

        // Write PIF ROM to PIF RAM
        for (i, &instr) in pif_rom.iter().enumerate() {
            let offset = i * 4;
            if offset + 3 < self.ram.len() {
                let bytes = instr.to_be_bytes();
                self.ram[offset] = bytes[0];
                self.ram[offset + 1] = bytes[1];
                self.ram[offset + 2] = bytes[2];
                self.ram[offset + 3] = bytes[3];
            }
        }
    }

    /// Read from PIF RAM
    pub fn read_ram(&self, offset: u32) -> u8 {
        let addr = (offset & 0x7FF) as usize;
        self.ram[addr]
    }

    /// Write to PIF RAM
    pub fn write_ram(&mut self, offset: u32, value: u8) {
        let addr = (offset & 0x7FF) as usize;
        self.ram[addr] = value;

        // Check if this is a controller command write (PIF RAM offset 0x7C0-0x7FF)
        // This is where games write controller command blocks
        if addr >= 0x7C0 {
            self.process_controller_commands();
        }
    }

    /// Process controller command blocks in PIF RAM
    fn process_controller_commands(&mut self) {
        // Command block format in PIF RAM (at 0x7C0+):
        // Each channel has: [T, R, command bytes...] where T=transmit, R=receive
        // Simplified implementation: just look for read controller command (0x01)

        // Controller 1 command at offset 0x7C0
        if self.ram[0x7C0] == 0x01 && self.ram[0x7C1] == 0x04 && self.ram[0x7C2] == 0x01 {
            // Command: 1 byte transmit, 4 bytes receive, read controller state
            let state = self.controller1; // Copy state to avoid borrow issues
            self.write_controller_state(0x7C3, &state);
        }

        // Controller 2 command at offset 0x7C8 (8 bytes per channel)
        if self.ram[0x7C8] == 0x01 && self.ram[0x7C9] == 0x04 && self.ram[0x7CA] == 0x01 {
            let state = self.controller2;
            self.write_controller_state(0x7CB, &state);
        }

        // Controller 3 and 4 similar (not implemented yet - rarely used)
    }

    /// Write controller state to PIF RAM response block
    fn write_controller_state(&mut self, offset: usize, state: &ControllerState) {
        // Response format: [buttons_hi, buttons_lo, stick_x, stick_y]
        let buttons = state.buttons.to_u16();
        self.ram[offset] = (buttons >> 8) as u8; // High byte
        self.ram[offset + 1] = (buttons & 0xFF) as u8; // Low byte
        self.ram[offset + 2] = state.stick_x as u8;
        self.ram[offset + 3] = state.stick_y as u8;
    }

    /// Update controller 1 state
    pub fn set_controller1(&mut self, state: ControllerState) {
        self.controller1 = state;
    }

    /// Update controller 2 state
    pub fn set_controller2(&mut self, state: ControllerState) {
        self.controller2 = state;
    }

    /// Update controller 3 state
    pub fn set_controller3(&mut self, state: ControllerState) {
        self.controller3 = state;
    }

    /// Update controller 4 state
    pub fn set_controller4(&mut self, state: ControllerState) {
        self.controller4 = state;
    }

    /// Get controller 1 state (for testing/debugging)
    #[allow(dead_code)]
    pub fn controller1(&self) -> &ControllerState {
        &self.controller1
    }
}

impl Default for Pif {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pif_creation() {
        let pif = Pif::new();
        assert!(!pif.controller1.buttons.a);
        assert_eq!(pif.controller1.stick_x, 0);
    }

    #[test]
    fn test_controller_buttons_packing() {
        let mut buttons = ControllerButtons::default();
        assert_eq!(buttons.to_u16(), 0);

        buttons.a = true;
        assert_eq!(buttons.to_u16(), 1 << 15);

        buttons.start = true;
        assert_eq!(buttons.to_u16(), (1 << 15) | (1 << 12));

        buttons.c_right = true;
        assert_eq!(buttons.to_u16(), (1 << 15) | (1 << 12) | 1);
    }

    #[test]
    fn test_ram_access() {
        let mut pif = Pif::new();

        pif.write_ram(0x100, 0x42);
        assert_eq!(pif.read_ram(0x100), 0x42);

        // Test wrapping
        pif.write_ram(0x900, 0x55); // Should wrap to 0x100
        assert_eq!(pif.read_ram(0x100), 0x55);
    }

    #[test]
    fn test_controller_state_write() {
        let mut pif = Pif::new();

        // Set controller state
        let mut state = ControllerState::default();
        state.buttons.a = true;
        state.buttons.start = true;
        state.stick_x = 64;
        state.stick_y = -32;
        pif.set_controller1(state);

        // Simulate game writing controller read command
        pif.write_ram(0x7C0, 0x01); // T=1 byte
        pif.write_ram(0x7C1, 0x04); // R=4 bytes
        pif.write_ram(0x7C2, 0x01); // Command 0x01 (read controller)

        // Response should be written at 0x7C3
        let buttons_hi = pif.read_ram(0x7C3);
        let buttons_lo = pif.read_ram(0x7C4);
        let stick_x = pif.read_ram(0x7C5) as i8;
        let stick_y = pif.read_ram(0x7C6) as i8;

        // Check button bits
        assert_eq!(buttons_hi, 0x90); // Bits 15 (A) and 12 (Start) set
        assert_eq!(buttons_lo, 0x00);
        assert_eq!(stick_x, 64);
        assert_eq!(stick_y, -32);
    }

    #[test]
    fn test_init_rom() {
        let mut pif = Pif::new();
        pif.init_rom();

        // Check that PIF ROM was written
        assert_ne!(pif.read_ram(0), 0);

        // Check first instruction (lui $t0, 0x9000)
        let instr = u32::from_be_bytes([
            pif.read_ram(0),
            pif.read_ram(1),
            pif.read_ram(2),
            pif.read_ram(3),
        ]);
        assert_eq!(instr, 0x3C089000);
    }

    #[test]
    fn test_multiple_controllers() {
        let mut pif = Pif::new();

        // Set controller 1
        let mut state1 = ControllerState::default();
        state1.buttons.a = true;
        pif.set_controller1(state1);

        // Set controller 2
        let mut state2 = ControllerState::default();
        state2.buttons.b = true;
        pif.set_controller2(state2);

        // Read controller 1 state
        pif.write_ram(0x7C0, 0x01);
        pif.write_ram(0x7C1, 0x04);
        pif.write_ram(0x7C2, 0x01);

        let buttons1 = u16::from_be_bytes([pif.read_ram(0x7C3), pif.read_ram(0x7C4)]);
        assert_eq!(buttons1 & (1 << 15), 1 << 15); // A button

        // Read controller 2 state
        pif.write_ram(0x7C8, 0x01);
        pif.write_ram(0x7C9, 0x04);
        pif.write_ram(0x7CA, 0x01);

        let buttons2 = u16::from_be_bytes([pif.read_ram(0x7CB), pif.read_ram(0x7CC)]);
        assert_eq!(buttons2 & (1 << 14), 1 << 14); // B button
    }

    #[test]
    fn test_button_state_active_high() {
        // Verify that N64 uses active-high logic (1 = pressed)
        let mut buttons = ControllerButtons::default();

        // No buttons pressed = all zeros
        assert_eq!(buttons.to_u16(), 0x0000);

        // Press A button (bit 15)
        buttons.a = true;
        assert_eq!(buttons.to_u16(), 0x8000);

        // Press multiple buttons
        buttons.b = true; // bit 14
        buttons.start = true; // bit 12
        buttons.d_up = true; // bit 11
        assert_eq!(buttons.to_u16(), 0xD800); // 1101 1000 0000 0000

        // Press all D-pad buttons
        buttons.d_down = true; // bit 10
        buttons.d_left = true; // bit 9
        buttons.d_right = true; // bit 8
        assert_eq!(buttons.to_u16() & 0x0F00, 0x0F00);

        // Press L and R triggers
        buttons.l = true; // bit 5
        buttons.r = true; // bit 4
        assert_eq!(buttons.to_u16() & 0x0030, 0x0030);

        // Press all C buttons
        buttons.c_up = true; // bit 3
        buttons.c_down = true; // bit 2
        buttons.c_left = true; // bit 1
        buttons.c_right = true; // bit 0
        assert_eq!(buttons.to_u16() & 0x000F, 0x000F);
    }

    #[test]
    fn test_analog_stick_range() {
        // Verify analog stick uses signed 8-bit range
        let mut state = ControllerState::default();

        // Center position
        assert_eq!(state.stick_x, 0);
        assert_eq!(state.stick_y, 0);

        // Full right/up
        state.stick_x = 127;
        state.stick_y = 127;
        assert_eq!(state.stick_x, 127);
        assert_eq!(state.stick_y, 127);

        // Full left/down
        state.stick_x = -128;
        state.stick_y = -128;
        assert_eq!(state.stick_x, -128);
        assert_eq!(state.stick_y, -128);
    }
}
