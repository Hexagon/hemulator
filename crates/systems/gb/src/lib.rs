//! Game Boy system implementation

use emu_core::{cpu_lr35902::CpuLr35902, types::Frame, MountPointInfo, System};

mod bus;
pub(crate) mod ppu;

use bus::GbBus;

pub struct GbSystem {
    cpu: CpuLr35902<GbBus>,
    cart_loaded: bool,
}

impl Default for GbSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GbSystem {
    pub fn new() -> Self {
        let bus = GbBus::new();
        let mut cpu = CpuLr35902::new(bus);
        cpu.reset();

        Self {
            cpu,
            cart_loaded: false,
        }
    }

    /// Set controller state (Game Boy buttons)
    /// Bits: 0=Right, 1=Left, 2=Up, 3=Down, 4=A, 5=B, 6=Select, 7=Start
    pub fn set_controller(&mut self, state: u8) {
        self.cpu.memory.set_buttons(state);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GbError {
    #[error("No cartridge loaded")]
    NoCartridge,
    #[error("Invalid mount point")]
    InvalidMountPoint,
}

impl System for GbSystem {
    type Error = GbError;

    fn reset(&mut self) {
        self.cpu.reset();
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        if !self.cart_loaded {
            return Err(GbError::NoCartridge);
        }

        // Game Boy runs at ~4.194304 MHz
        // Frame rate is ~59.73 Hz
        // Cycles per frame: 4194304 / 59.73 ≈ 70224 cycles
        const CYCLES_PER_FRAME: u32 = 70224;

        let mut cycles = 0;
        while cycles < CYCLES_PER_FRAME {
            let cpu_cycles = self.cpu.step();
            cycles += cpu_cycles;

            // Step PPU
            if self.cpu.memory.ppu.step(cpu_cycles) {
                // V-Blank started - could trigger NMI here
            }
        }

        // Render the frame from PPU
        Ok(self.cpu.memory.ppu.render_frame())
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({
            "system": "gb",
            "version": 1,
            "cpu": {
                "a": self.cpu.a,
                "f": self.cpu.f,
                "b": self.cpu.b,
                "c": self.cpu.c,
                "d": self.cpu.d,
                "e": self.cpu.e,
                "h": self.cpu.h,
                "l": self.cpu.l,
                "sp": self.cpu.sp,
                "pc": self.cpu.pc,
                "ime": self.cpu.ime,
                "halted": self.cpu.halted,
                "stopped": self.cpu.stopped,
            }
        })
    }

    fn load_state(&mut self, v: &serde_json::Value) -> Result<(), serde_json::Error> {
        macro_rules! load_u8 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u8;
                }
            };
        }

        macro_rules! load_u16 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u16;
                }
            };
        }

        macro_rules! load_bool {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_bool()) {
                    $target = val;
                }
            };
        }

        if let Some(cpu_state) = v.get("cpu") {
            load_u8!(cpu_state, "a", self.cpu.a);
            load_u8!(cpu_state, "f", self.cpu.f);
            load_u8!(cpu_state, "b", self.cpu.b);
            load_u8!(cpu_state, "c", self.cpu.c);
            load_u8!(cpu_state, "d", self.cpu.d);
            load_u8!(cpu_state, "e", self.cpu.e);
            load_u8!(cpu_state, "h", self.cpu.h);
            load_u8!(cpu_state, "l", self.cpu.l);
            load_u16!(cpu_state, "sp", self.cpu.sp);
            load_u16!(cpu_state, "pc", self.cpu.pc);
            load_bool!(cpu_state, "ime", self.cpu.ime);
            load_bool!(cpu_state, "halted", self.cpu.halted);
            load_bool!(cpu_state, "stopped", self.cpu.stopped);
        }
        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        true
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Cartridge".to_string(),
            name: "Cartridge Slot".to_string(),
            extensions: vec!["gb".to_string(), "gbc".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(GbError::InvalidMountPoint);
        }

        self.cpu.memory.load_cart(data);
        self.cart_loaded = true;
        self.reset();

        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(GbError::InvalidMountPoint);
        }

        self.cart_loaded = false;
        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "Cartridge" && self.cart_loaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gb_system_creation() {
        let sys = GbSystem::new();
        assert!(!sys.cart_loaded);
    }

    #[test]
    fn test_gb_mount_points() {
        let sys = GbSystem::new();
        let mount_points = sys.mount_points();
        assert_eq!(mount_points.len(), 1);
        assert_eq!(mount_points[0].id, "Cartridge");
        assert!(mount_points[0].required);
    }

    #[test]
    fn test_gb_mount_unmount() {
        let mut sys = GbSystem::new();
        assert!(!sys.is_mounted("Cartridge"));

        // Mount a minimal ROM
        let rom = vec![0; 0x8000]; // 32KB ROM
        assert!(sys.mount("Cartridge", &rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        assert!(sys.unmount("Cartridge").is_ok());
        assert!(!sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_gb_save_load_state() {
        let sys = GbSystem::new();
        let state = sys.save_state();
        assert_eq!(state["system"], "gb");
        assert_eq!(state["version"], 1);

        let mut sys2 = GbSystem::new();
        assert!(sys2.load_state(&state).is_ok());
    }

    #[test]
    fn test_gb_supports_save_states() {
        let sys = GbSystem::new();
        assert!(sys.supports_save_states());
    }

    #[test]
    fn test_gb_step_frame_without_cart() {
        let mut sys = GbSystem::new();
        let result = sys.step_frame();
        assert!(result.is_err());
    }

    #[test]
    fn test_gb_step_frame_with_cart() {
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        let result = sys.step_frame();
        assert!(result.is_ok());
        let frame = result.unwrap();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_gb_controller_input() {
        let mut sys = GbSystem::new();
        
        // Test setting controller state
        sys.set_controller(0xFF); // All buttons released
        
        // Test individual buttons
        sys.set_controller(0x01); // Right pressed
        sys.set_controller(0x10); // A pressed
        sys.set_controller(0x80); // Start pressed
    }

    #[test]
    fn test_gb_ppu_registers() {
        let sys = GbSystem::new();
        
        // Verify initial PPU register values
        assert_eq!(sys.cpu.memory.ppu.lcdc, 0x91);
        assert_eq!(sys.cpu.memory.ppu.bgp, 0xFC);
        assert_eq!(sys.cpu.memory.ppu.ly, 0);
    }
}
