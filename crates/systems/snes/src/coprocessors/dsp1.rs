//! DSP-1 Math Coprocessor Implementation
//!
//! The DSP-1 is a math coprocessor used in games like Pilotwings and Super Mario Kart.
//! It provides various mathematical operations including:
//! - Multiplication and division
//! - Matrix transformations (rotation, scaling)
//! - Trigonometric functions (sine, cosine)
//! - Distance calculations
//! - Coordinate transformations
//!
//! ## Memory Mapping
//!
//! In LoROM games (most DSP-1 games):
//! - $3000-$3FFF in banks $30-$3F: Data Register (DR)
//! - $7000-$7FFF in banks $30-$3F: Status Register (SR)
//!
//! In HiROM games:
//! - $6000-$6FFF in banks $00-$1F: DR
//! - $7000-$7FFF in banks $00-$1F: SR
//!
//! ## References
//!
//! - https://snes.nesdev.org/wiki/DSP-1
//! - https://sneslab.net/wiki/DSP1
//! - https://problemkaputt.de/fullsnes.htm#snesextdspdsp1dsp1adsp1b

use super::{ChipType, EnhancementChip};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// DSP-1 command codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Dsp1Command {
    /// Multiply 16-bit values
    Multiply = 0x00,
    /// Multiply and accumulate
    MultiplyAccumulate = 0x01,
    /// Inverse (1/x)
    Inverse = 0x04,
    /// Attitude (rotation/scaling)
    Attitude = 0x08,
    /// Gyrate (2D rotation)
    Gyrate = 0x0C,
    /// Project (3D projection)
    Project = 0x10,
    /// Radius (distance calculation)
    Radius = 0x14,
    /// Range (3D distance)
    Range = 0x18,
    /// Distance (2D distance)
    Distance = 0x1C,
    /// Target (coordinate transformation)
    Target = 0x20,
    /// Rotate (3D rotation)
    Rotate = 0x24,
    /// Polar to Cartesian conversion
    Polar = 0x28,
    /// Unknown command
    Unknown = 0xFF,
}

impl From<u8> for Dsp1Command {
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::Multiply,
            0x01 => Self::MultiplyAccumulate,
            0x04 => Self::Inverse,
            0x08 => Self::Attitude,
            0x0C => Self::Gyrate,
            0x10 => Self::Project,
            0x14 => Self::Radius,
            0x18 => Self::Range,
            0x1C => Self::Distance,
            0x20 => Self::Target,
            0x24 => Self::Rotate,
            0x28 => Self::Polar,
            _ => Self::Unknown,
        }
    }
}

/// DSP-1 state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Dsp1State {
    /// Waiting for command
    WaitingForCommand,
    /// Reading parameters
    ReadingParameters,
    /// Computing result
    Computing,
    /// Writing output
    WritingOutput,
}

/// DSP-1 Math Coprocessor
#[derive(Clone, Serialize, Deserialize)]
pub struct Dsp1 {
    /// Current state
    state: Dsp1State,
    /// Current command being executed
    command: u8,
    /// Input parameter buffer
    input_buffer: Vec<u8>,
    /// Output result buffer
    output_buffer: Vec<u8>,
    /// Current position in input buffer
    input_pos: usize,
    /// Current position in output buffer
    output_pos: usize,
    /// Number of parameters expected for current command
    expected_params: usize,
    /// Number of output bytes for current command
    output_size: usize,
}

impl Default for Dsp1 {
    fn default() -> Self {
        Self {
            state: Dsp1State::WaitingForCommand,
            command: 0,
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            input_pos: 0,
            output_pos: 0,
            expected_params: 0,
            output_size: 0,
        }
    }
}

impl Dsp1 {
    /// Create a new DSP-1 instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of input parameters required for a command
    fn get_param_count(command: Dsp1Command) -> usize {
        match command {
            Dsp1Command::Multiply => 4,           // 2x 16-bit values
            Dsp1Command::MultiplyAccumulate => 4, // 2x 16-bit values
            Dsp1Command::Inverse => 2,            // 1x 16-bit value
            Dsp1Command::Attitude => 8,           // 4x 16-bit angles
            Dsp1Command::Gyrate => 6,             // 3x 16-bit values
            Dsp1Command::Project => 6,            // 3x 16-bit coordinates
            Dsp1Command::Radius => 6,             // 3x 16-bit coordinates
            Dsp1Command::Range => 6,              // 3x 16-bit coordinates
            Dsp1Command::Distance => 4,           // 2x 16-bit coordinates
            Dsp1Command::Target => 4,             // 2x 16-bit values (H, V)
            Dsp1Command::Rotate => 6,             // 3x 16-bit values (angle, x, y)
            Dsp1Command::Polar => 6,              // 3x 16-bit values
            Dsp1Command::Unknown => 0,
        }
    }

    /// Get the number of output bytes for a command
    fn get_output_size(command: Dsp1Command) -> usize {
        match command {
            Dsp1Command::Multiply => 4,           // 1x 32-bit result
            Dsp1Command::MultiplyAccumulate => 4, // 1x 32-bit result
            Dsp1Command::Inverse => 2,            // 1x 16-bit result
            Dsp1Command::Attitude => 8,           // 4x 16-bit sin/cos values (simplified)
            Dsp1Command::Gyrate => 4,             // 2x 16-bit coordinates
            Dsp1Command::Project => 4,            // 2x 16-bit screen coordinates
            Dsp1Command::Radius => 2,             // 1x 16-bit distance
            Dsp1Command::Range => 2,              // 1x 16-bit distance
            Dsp1Command::Distance => 2,           // 1x 16-bit distance
            Dsp1Command::Target => 4,             // 2x 16-bit values (simplified)
            Dsp1Command::Rotate => 4,             // 2x 16-bit coordinates (simplified)
            Dsp1Command::Polar => 4,              // 2x 16-bit coordinates
            Dsp1Command::Unknown => 0,
        }
    }

    /// Read a 16-bit signed value from input buffer
    fn read_s16(&self, offset: usize) -> i16 {
        if offset + 1 < self.input_buffer.len() {
            i16::from_le_bytes([self.input_buffer[offset], self.input_buffer[offset + 1]])
        } else {
            0
        }
    }

    /// Write a 16-bit signed value to output buffer
    fn write_s16(&mut self, offset: usize, value: i16) {
        let bytes = value.to_le_bytes();
        if offset + 1 < self.output_buffer.len() {
            self.output_buffer[offset] = bytes[0];
            self.output_buffer[offset + 1] = bytes[1];
        }
    }

    /// Write a 32-bit signed value to output buffer
    fn write_s32(&mut self, offset: usize, value: i32) {
        let bytes = value.to_le_bytes();
        if offset + 3 < self.output_buffer.len() {
            self.output_buffer[offset] = bytes[0];
            self.output_buffer[offset + 1] = bytes[1];
            self.output_buffer[offset + 2] = bytes[2];
            self.output_buffer[offset + 3] = bytes[3];
        }
    }

    /// Execute the current command
    fn execute_command(&mut self) {
        let cmd = Dsp1Command::from(self.command);

        match cmd {
            Dsp1Command::Multiply => {
                let a = self.read_s16(0) as i32;
                let b = self.read_s16(2) as i32;
                self.write_s32(0, a * b);
            }
            Dsp1Command::MultiplyAccumulate => {
                // Same as multiply for now (accumulation would require state)
                let a = self.read_s16(0) as i32;
                let b = self.read_s16(2) as i32;
                self.write_s32(0, a * b);
            }
            Dsp1Command::Inverse => {
                let value = self.read_s16(0);
                let result = if value == 0 {
                    0x7FFF // Maximum positive value on divide by zero
                } else {
                    ((0x10000i32) / (value as i32)) as i16
                };
                self.write_s16(0, result);
            }
            Dsp1Command::Attitude => {
                // Attitude command - compute 3x3 rotation matrix
                // Based on bsnes implementation (attitudeA/B/C)
                // Input: S (scale), Rz, Ry, Rx (rotation angles for Z, Y, X axes)
                // Output: Currently simplified to 4 values instead of full 9-element matrix
                //
                // The bsnes implementation shows this creates an "attitude matrix":
                //           S | cosRz  sinRz    0| |cosRy   0   -sinRy| |1      0      0  |
                // MatrixA = - |-sinRz  cosRz    0| |  0     1      0  | |0    cosRx  sinRx|
                //           2 |   0      0      1| |sinRy   0    cosRy| |0   -sinRx  cosRx|
                //
                // This simplified implementation stores the rotation angles' sin/cos values
                // For full hardware accuracy, would need to compute and store the 9 matrix
                // elements and maintain shared state for use by other commands
                //
                // References:
                // - https://sneslab.net/wiki/DSP1/Attitude
                // - bsnes/sfc/coprocessor/dsp1/dsp1emu.cpp (attitudeA function)
                for i in 0..4 {
                    let angle = self.read_s16(i * 2);
                    // Convert to radians: angle is in 1/256th of a full rotation
                    let radians = (angle as f64) * 2.0 * PI / 65536.0;
                    let sin_val = (radians.sin() * 32767.0) as i16;
                    self.write_s16(i * 2, sin_val);
                }
            }
            Dsp1Command::Gyrate => {
                // 2D rotation
                let x = self.read_s16(0) as f64;
                let y = self.read_s16(2) as f64;
                let angle = self.read_s16(4);
                let radians = (angle as f64) * 2.0 * PI / 65536.0;
                let cos_a = radians.cos();
                let sin_a = radians.sin();
                let new_x = (x * cos_a - y * sin_a) as i16;
                let new_y = (x * sin_a + y * cos_a) as i16;
                self.write_s16(0, new_x);
                self.write_s16(2, new_y);
            }
            Dsp1Command::Distance => {
                // 2D distance
                let x = self.read_s16(0) as f64;
                let y = self.read_s16(2) as f64;
                let dist = (x * x + y * y).sqrt() as i16;
                self.write_s16(0, dist);
            }
            Dsp1Command::Radius => {
                // 3D distance (same as Range)
                let x = self.read_s16(0) as f64;
                let y = self.read_s16(2) as f64;
                let z = self.read_s16(4) as f64;
                let dist = (x * x + y * y + z * z).sqrt() as i16;
                self.write_s16(0, dist);
            }
            Dsp1Command::Range => {
                // 3D distance
                let x = self.read_s16(0) as f64;
                let y = self.read_s16(2) as f64;
                let z = self.read_s16(4) as f64;
                let dist = (x * x + y * y + z * z).sqrt() as i16;
                self.write_s16(0, dist);
            }
            Dsp1Command::Project => {
                // Simple 3D projection (simplified)
                let x = self.read_s16(0);
                let y = self.read_s16(2);
                let z = self.read_s16(4);
                // Perspective divide by z (with safety check)
                let screen_x = if z != 0 {
                    (x as i32 * 256) / z as i32
                } else {
                    x as i32
                };
                let screen_y = if z != 0 {
                    (y as i32 * 256) / z as i32
                } else {
                    y as i32
                };
                self.write_s16(0, screen_x as i16);
                self.write_s16(2, screen_y as i16);
            }
            Dsp1Command::Polar => {
                // Polar to Cartesian
                let radius = self.read_s16(0) as f64;
                let angle = self.read_s16(2);
                let radians = (angle as f64) * 2.0 * PI / 65536.0;
                let x = (radius * radians.cos()) as i16;
                let y = (radius * radians.sin()) as i16;
                self.write_s16(0, x);
                self.write_s16(2, y);
            }
            Dsp1Command::Rotate => {
                // Rotate command - 3D point rotation
                // Based on bsnes implementation (rotate function)
                // Input: Angle, X1, Y1 (angle and 2D coordinates)
                // Output: X2, Y2 (rotated 2D coordinates)
                //
                // Formula (clockwise rotation):
                // X2 = (Y1 * sin(Angle)) + (X1 * cos(Angle))
                // Y2 = (Y1 * cos(Angle)) - (X1 * sin(Angle))
                //
                // Reference: bsnes/sfc/coprocessor/dsp1/dsp1emu.cpp
                let angle = self.read_s16(0);
                let x1 = self.read_s16(2) as f64;
                let y1 = self.read_s16(4) as f64;

                let radians = (angle as f64) * 2.0 * PI / 65536.0;
                let sin_a = radians.sin();
                let cos_a = radians.cos();

                // Perform 2D rotation (results are fixed-point scaled by 32767)
                let x2 = ((y1 * sin_a) + (x1 * cos_a)) as i16;
                let y2 = ((y1 * cos_a) - (x1 * sin_a)) as i16;

                self.write_s16(0, x2);
                self.write_s16(2, y2);
            }
            Dsp1Command::Target => {
                // Target command - screen to ground coordinate projection
                // Based on bsnes implementation (target function)
                //
                // NOTE: This command requires projection parameters set by the Parameter command (0x02).
                // The full implementation needs shared state for projection matrices and camera parameters.
                // Current simplified implementation provides basic coordinate transformation.
                //
                // Input: H, V (horizontal and vertical screen coordinates)
                // Output: X, Y (ground coordinates)
                //
                // Reference: bsnes/sfc/coprocessor/dsp1/dsp1emu.cpp (target function)
                // The actual formula involves projection matrices, center of projection,
                // azimuth/zenith angles, and normalized calculations with inverse operations.
                //
                // For now, provide a simplified pass-through transformation
                // Full implementation would require:
                // - Shared projection matrix state
                // - Parameter command (0x02) to set up projection
                // - Complex inverse and normalization operations
                let h = self.read_s16(0);
                let v = self.read_s16(2);

                // Simplified transformation (identity-like, scaled)
                // Real hardware uses complex projection math with shared state
                let x = h;
                let y = v;

                self.write_s16(0, x);
                self.write_s16(2, y);
            }
            Dsp1Command::Unknown => {
                // Unknown/unsupported command - return zeros
                for i in 0..self.output_buffer.len() {
                    self.output_buffer[i] = 0;
                }
            }
        }
    }

    /// Write a byte to the data register
    fn write_data(&mut self, value: u8) {
        match self.state {
            Dsp1State::WaitingForCommand => {
                // Receiving command byte
                self.command = value;
                let cmd = Dsp1Command::from(value);
                self.expected_params = Self::get_param_count(cmd);
                self.output_size = Self::get_output_size(cmd);
                self.input_buffer = vec![0; self.expected_params];
                self.output_buffer = vec![0; self.output_size];
                self.input_pos = 0;
                self.output_pos = 0;

                if self.expected_params == 0 {
                    // No parameters needed, execute immediately
                    self.execute_command();
                    self.state = Dsp1State::WritingOutput;
                } else {
                    self.state = Dsp1State::ReadingParameters;
                }
            }
            Dsp1State::ReadingParameters => {
                // Receiving parameter bytes
                if self.input_pos < self.input_buffer.len() {
                    self.input_buffer[self.input_pos] = value;
                    self.input_pos += 1;

                    if self.input_pos >= self.expected_params {
                        // All parameters received, execute command
                        self.execute_command();
                        self.state = Dsp1State::WritingOutput;
                    }
                }
            }
            _ => {
                // Ignore writes in other states
            }
        }
    }

    /// Read a byte from the data register
    fn read_data(&mut self) -> u8 {
        match self.state {
            Dsp1State::WritingOutput => {
                if self.output_pos < self.output_buffer.len() {
                    let value = self.output_buffer[self.output_pos];
                    self.output_pos += 1;

                    if self.output_pos >= self.output_size {
                        // All output read, ready for next command
                        self.state = Dsp1State::WaitingForCommand;
                    }

                    value
                } else {
                    0
                }
            }
            _ => {
                // Return 0 if reading while not in output state
                0
            }
        }
    }

    /// Read the status register
    fn read_status(&self) -> u8 {
        match self.state {
            Dsp1State::WaitingForCommand => 0x80, // Ready for command
            Dsp1State::ReadingParameters => 0x00, // Busy reading parameters
            Dsp1State::Computing => 0x00,         // Busy computing
            Dsp1State::WritingOutput => 0x80,     // Ready with output
        }
    }
}

impl EnhancementChip for Dsp1 {
    fn read(&mut self, addr: u32) -> u8 {
        let offset = (addr & 0xFFFF) as u16;

        // Check if reading from status register area ($7000-$7FFF in LoROM banks $30-$3F)
        // or data register area ($3000-$3FFF in LoROM banks $30-$3F)
        if offset >= 0x7000 {
            // Status register
            self.read_status()
        } else if offset >= 0x3000 {
            // Data register
            self.read_data()
        } else {
            0
        }
    }

    fn write(&mut self, addr: u32, value: u8) {
        let offset = (addr & 0xFFFF) as u16;

        // Data register is at $3000-$3FFF in LoROM banks $30-$3F
        if (0x3000..0x7000).contains(&offset) {
            self.write_data(value);
        }
        // Status register is read-only
    }

    fn reset(&mut self) {
        self.state = Dsp1State::WaitingForCommand;
        self.command = 0;
        self.input_buffer.clear();
        self.output_buffer.clear();
        self.input_pos = 0;
        self.output_pos = 0;
        self.expected_params = 0;
        self.output_size = 0;
    }

    fn chip_type(&self) -> ChipType {
        ChipType::Dsp1
    }

    fn save_state(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| format!("Failed to serialize DSP-1 state: {}", e))
    }

    fn load_state(&mut self, state: &str) -> Result<(), String> {
        // Deserialize and validate the state
        let loaded: Dsp1 = serde_json::from_str(state)
            .map_err(|e| format!("Failed to deserialize DSP-1 state: {}", e))?;

        // Validate state is reasonable (basic sanity checks)
        if loaded.input_pos > loaded.input_buffer.len()
            || loaded.output_pos > loaded.output_buffer.len()
        {
            return Err("Invalid DSP-1 state: buffer positions out of bounds".to_string());
        }

        // State is valid, replace current state
        *self = loaded;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsp1_multiply() {
        let mut dsp = Dsp1::new();

        // Status should indicate ready
        assert_eq!(dsp.read_status(), 0x80);

        // Send multiply command (0x00)
        dsp.write_data(0x00);

        // Send parameters: 100 (0x0064) and 200 (0x00C8)
        dsp.write_data(0x64); // low byte of first param
        dsp.write_data(0x00); // high byte of first param
        dsp.write_data(0xC8); // low byte of second param
        dsp.write_data(0x00); // high byte of second param

        // Should now be in output state
        assert_eq!(dsp.state, Dsp1State::WritingOutput);

        // Read result (100 * 200 = 20000 = 0x00004E20)
        let b0 = dsp.read_data();
        let b1 = dsp.read_data();
        let b2 = dsp.read_data();
        let b3 = dsp.read_data();

        let result = i32::from_le_bytes([b0, b1, b2, b3]);
        assert_eq!(result, 20000);

        // Should be back to waiting for command
        assert_eq!(dsp.state, Dsp1State::WaitingForCommand);
    }

    #[test]
    fn test_dsp1_inverse() {
        let mut dsp = Dsp1::new();

        // Send inverse command (0x04)
        dsp.write_data(0x04);

        // Send parameter: 100 (0x0064)
        dsp.write_data(0x64); // low byte
        dsp.write_data(0x00); // high byte

        // Read result
        let b0 = dsp.read_data();
        let b1 = dsp.read_data();

        let result = i16::from_le_bytes([b0, b1]);
        // Result should be approximately 0x10000 / 100 = 655
        assert!((result as i32 - 655).abs() <= 1);
    }

    #[test]
    fn test_dsp1_distance() {
        let mut dsp = Dsp1::new();

        // Send distance command (0x1C)
        dsp.write_data(0x1C);

        // Send parameters: x=3, y=4
        dsp.write_data(0x03); // low byte of x
        dsp.write_data(0x00); // high byte of x
        dsp.write_data(0x04); // low byte of y
        dsp.write_data(0x00); // high byte of y

        // Read result (should be 5, since 3^2 + 4^2 = 25, sqrt(25) = 5)
        let b0 = dsp.read_data();
        let b1 = dsp.read_data();

        let result = i16::from_le_bytes([b0, b1]);
        assert_eq!(result, 5);
    }

    #[test]
    fn test_dsp1_divide_by_zero() {
        let mut dsp = Dsp1::new();

        // Send inverse command (0x04)
        dsp.write_data(0x04);

        // Send parameter: 0
        dsp.write_data(0x00);
        dsp.write_data(0x00);

        // Read result (should be max value, not crash)
        let b0 = dsp.read_data();
        let b1 = dsp.read_data();

        let result = i16::from_le_bytes([b0, b1]);
        assert_eq!(result, 0x7FFF); // Maximum positive value
    }

    #[test]
    fn test_dsp1_save_load_state() {
        let mut dsp = Dsp1::new();

        // Set up some state
        dsp.write_data(0x00); // Multiply command
        dsp.write_data(0x64);
        dsp.write_data(0x00);

        // Save state
        let state = dsp.save_state().unwrap();

        // Create new instance and load state
        let mut dsp2 = Dsp1::new();
        dsp2.load_state(&state).unwrap();

        // Verify state was restored
        assert_eq!(dsp2.state, dsp.state);
        assert_eq!(dsp2.command, dsp.command);
        assert_eq!(dsp2.input_pos, dsp.input_pos);
        assert_eq!(dsp2.input_buffer, dsp.input_buffer);
    }

    #[test]
    fn test_dsp1_gyrate() {
        let mut dsp = Dsp1::new();

        // Send gyrate command (0x0C) - 2D rotation
        dsp.write_data(0x0C);

        // Send parameters: x=100, y=0, angle=0 (should be identity)
        dsp.write_data(0x64); // x low
        dsp.write_data(0x00); // x high
        dsp.write_data(0x00); // y low
        dsp.write_data(0x00); // y high
        dsp.write_data(0x00); // angle low
        dsp.write_data(0x00); // angle high

        // Read result (at 0 degrees, should be roughly x=100, y=0)
        let x_low = dsp.read_data();
        let x_high = dsp.read_data();
        let y_low = dsp.read_data();
        let y_high = dsp.read_data();

        let x = i16::from_le_bytes([x_low, x_high]);
        let y = i16::from_le_bytes([y_low, y_high]);

        // At 0 degrees, output should be close to input
        assert_eq!(x, 100);
        assert_eq!(y, 0);
    }

    #[test]
    fn test_dsp1_polar_to_cartesian() {
        let mut dsp = Dsp1::new();

        // Send polar command (0x28) - requires 6 bytes total input
        dsp.write_data(0x28);

        // Send parameters: radius=100, angle=0, plus extra parameter
        dsp.write_data(0x64); // radius low
        dsp.write_data(0x00); // radius high
        dsp.write_data(0x00); // angle low
        dsp.write_data(0x00); // angle high
        dsp.write_data(0x00); // extra param low (DSP-1 uses 3x 16-bit for polar)
        dsp.write_data(0x00); // extra param high

        // Read result (should be 4 bytes: x, y coordinates)
        let x_low = dsp.read_data();
        let x_high = dsp.read_data();
        let y_low = dsp.read_data();
        let y_high = dsp.read_data();

        let x = i16::from_le_bytes([x_low, x_high]);
        let y = i16::from_le_bytes([y_low, y_high]);

        // At angle 0, x should be ~100, y should be ~0
        assert!(x > 90 && x < 110);
        assert!(y.abs() < 10);
    }
}
