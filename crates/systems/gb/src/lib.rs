//! Minimal Game Boy system skeleton for wiring into the core.

use emu_core::{types::Frame, System, MountPointInfo};

#[derive(Debug, Default)]
pub struct GbSystem {
    // placeholder fields for CPU, PPU (LCD), MMU, cartridge
}

#[derive(thiserror::Error, Debug)]
#[error("GB error")]
pub struct GbError;

impl System for GbSystem {
    type Error = GbError;

    fn reset(&mut self) {}

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Game Boy native framebuffer is 160x144
        Ok(Frame::new(160, 144))
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({ "system": "gb", "version": 1 })
    }

    fn load_state(&mut self, _v: &serde_json::Value) -> Result<(), serde_json::Error> {
        Ok(())
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Cartridge".to_string(),
            name: "Cartridge Slot".to_string(),
            extensions: vec!["gb".to_string(), "gbc".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, _data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(GbError);
        }
        // Skeleton implementation - not yet functional
        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(GbError);
        }
        Ok(())
    }

    fn is_mounted(&self, _mount_point_id: &str) -> bool {
        false
    }
}
