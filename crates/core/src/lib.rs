//! Core emulator primitives and traits.

pub mod apu;
pub mod cpu_6502;
pub mod types {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Frame {
        pub width: u32,
        pub height: u32,
        pub pixels: Vec<u32>,
    }

    impl Frame {
        pub fn new(width: u32, height: u32) -> Self {
            Self {
                width,
                height,
                pixels: vec![0; (width * height) as usize],
            }
        }
    }

    pub type AudioSample = i16;
}

use serde_json::Value;

/// A CPU-like component that can be stepped; returns cycles consumed.
pub trait Cpu {
    fn reset(&mut self);
    fn step(&mut self) -> u32;
}

/// A high-level System trait tying components together.
pub trait System {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Reset to initial power-on state
    fn reset(&mut self);

    /// Emulate until a frame is produced and return a framebuffer.
    fn step_frame(&mut self) -> Result<types::Frame, Self::Error>;

    /// Return a JSON-serializable save state for debugging.
    fn save_state(&self) -> Value;

    /// Load a JSON save state.
    fn load_state(&mut self, v: &Value) -> Result<(), serde_json::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_initialization() {
        let f = types::Frame::new(10, 10);
        assert_eq!(f.pixels.len(), 100);
        assert_eq!(f.width, 10);
        assert_eq!(f.height, 10);
    }

    struct MockSystem;

    impl System for MockSystem {
        type Error = std::convert::Infallible;

        fn reset(&mut self) {}

        fn step_frame(&mut self) -> Result<types::Frame, Self::Error> {
            Ok(types::Frame::new(2, 2))
        }

        fn save_state(&self) -> serde_json::Value {
            serde_json::json!({"mock": true, "version": 1})
        }

        fn load_state(&mut self, _v: &serde_json::Value) -> Result<(), serde_json::Error> {
            Ok(())
        }
    }

    #[test]
    fn mock_system_save_load_roundtrip() {
        let sys = MockSystem;
        let v = sys.save_state();
        let s = serde_json::to_string(&v).expect("serialize");
        let v2: serde_json::Value = serde_json::from_str(&s).expect("deserialize");
        let mut sys2 = MockSystem;
        assert!(sys2.load_state(&v2).is_ok());
    }
}
