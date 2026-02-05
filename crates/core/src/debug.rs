//! Debug interface for system introspection and analysis.
//!
//! This module provides traits and types for debugging emulated systems,
//! including disassembly, memory inspection, and CPU state tracking.
//!
//! # Overview
//!
//! The debugger subsystem enables comprehensive introspection of emulated systems
//! through a unified interface. Every system implements the [`Debugger`] trait to
//! expose its internal state for debugging, testing, and analysis.
//!
//! # Core Components
//!
//! - [`Debugger`] trait: Main interface for system debugging
//! - [`MemoryRegion`]: Describes a memory region with metadata
//! - [`DisassembledInstruction`]: Represents a disassembled instruction
//! - [`CpuRegister`]: CPU register with name, value, and width
//! - [`CpuFlags`]: Collection of CPU status flags
//! - [`CpuState`]: Complete CPU state snapshot
//! - [`ExecutionTrace`]: Instruction execution record with post-execution state
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use emu_core::debug::Debugger;
//!
//! // Get CPU state
//! let state = system.get_cpu_state();
//! println!("PC: ${:04X}", state.pc);
//!
//! // Disassemble around current PC
//! let instructions = system.disassemble_range(state.pc, 10);
//! for instr in instructions {
//!     println!("{:04X}: {}", instr.address, instr.mnemonic);
//! }
//!
//! // Read memory
//! if let Some(data) = system.read_memory(0x0000, 256) {
//!     // Inspect memory contents
//! }
//!
//! // Get memory map
//! for region in system.get_memory_regions() {
//!     println!("{}: ${:04X}-${:04X} ({})",
//!         region.name, region.start, region.end, region.description);
//! }
//! ```
//!
//! # Implementation Guide
//!
//! When implementing the [`Debugger`] trait for a new system:
//!
//! 1. **Implement `disassemble_instruction`**:
//!    - Read instruction bytes via `read_memory`
//!    - Call appropriate disassembler function
//!    - Return `None` for invalid addresses
//!
//! 2. **Implement `read_memory`**:
//!    - Validate address bounds
//!    - Read from system bus/memory
//!    - Handle wrapping and mirroring
//!
//! 3. **Implement `get_memory_regions`**:
//!    - List all memory regions in address order
//!    - Include RAM, ROM, I/O registers, etc.
//!    - Set correct read/write permissions
//!
//! 4. **Implement `get_cpu_state`**:
//!    - Create `CpuState` with current PC
//!    - Add all registers (including PC for display)
//!    - Add all CPU flags in logical order
//!
//! 5. **Use helper macro for execution history**:
//!    - Add `instruction_tracer: InstructionTracer` field to system
//!    - Use `impl_debugger_execution_history!()` macro
//!    - Optionally use `impl_instruction_tracer_methods!()` for convenience
//!
//! # Example Implementation
//!
//! ```rust,ignore
//! impl Debugger for MySystem {
//!     fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
//!         let memory = self.read_memory(address, 3)?;
//!         disasm_mycpu::disassemble(&memory, address)
//!     }
//!
//!     fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
//!         if address > 0xFFFF {
//!             return None;
//!         }
//!         let mut result = Vec::with_capacity(length);
//!         for i in 0..length {
//!             let addr = address.wrapping_add(i as u32);
//!             if addr > 0xFFFF { break; }
//!             result.push(self.memory.read(addr as u16));
//!         }
//!         Some(result)
//!     }
//!
//!     fn get_memory_regions(&self) -> Vec<MemoryRegion> {
//!         vec![
//!             MemoryRegion::new("RAM", 0x0000, 0x1FFF, "System RAM", true, true),
//!             MemoryRegion::new("ROM", 0x8000, 0xFFFF, "Program ROM", true, false),
//!         ]
//!     }
//!
//!     fn get_cpu_state(&self) -> CpuState {
//!         let mut state = CpuState::new(self.cpu.pc as u32);
//!         state.add_register(CpuRegister::new_16bit("PC", self.cpu.pc));
//!         state.add_register(CpuRegister::new_8bit("A", self.cpu.a));
//!         state.add_flag("Z", (self.cpu.status & 0x02) != 0);
//!         state
//!     }
//!
//!     // Automatically implement execution history methods
//!     emu_core::impl_debugger_execution_history!();
//! }
//! ```
//!
//! # Advanced Features
//!
//! ## Instruction Tracing
//!
//! Systems can enable instruction tracing to record execution history:
//!
//! ```rust,ignore
//! // Enable tracing (typically via command-line flag)
//! system.set_instruction_tracing(true);
//!
//! // Run emulation
//! system.step_frame();
//!
//! // Access trace history
//! let history = system.get_execution_history();
//! for trace in history.iter().take(10) {
//!     println!("{:04X}: {}", trace.instruction.address, trace.instruction.mnemonic);
//! }
//! ```
//!
//! ## Mode-Specific Disassembly
//!
//! Some CPUs require mode tracking for accurate disassembly (e.g., 65C816 M/X flags).
//! Override `disassemble_range` to track mode changes:
//!
//! ```rust,ignore
//! fn disassemble_range(&self, address: u32, count: usize) -> Vec<DisassembledInstruction> {
//!     let mut result = Vec::new();
//!     let mut current_address = address;
//!     let mut m_flag = (self.cpu.status & 0x20) != 0;
//!     let mut x_flag = (self.cpu.status & 0x10) != 0;
//!
//!     for _ in 0..count {
//!         let memory = match self.read_memory(current_address, 4) {
//!             Some(m) => m,
//!             None => break,
//!         };
//!
//!         let (instruction, new_m, new_x) =
//!             disasm_65c816::disassemble_tracking_flags(&memory, current_address, m_flag, x_flag)?;
//!
//!         current_address += instruction.len() as u32;
//!         result.push(instruction);
//!
//!         m_flag = new_m;
//!         x_flag = new_x;
//!     }
//!     result
//! }
//! ```
//!
//! # Testing
//!
//! All `Debugger` implementations should include comprehensive tests:
//!
//! ```rust,ignore
//! #[cfg(test)]
//! mod tests {
//!     use super::*;
//!
//!     #[test]
//!     fn test_memory_regions() {
//!         let system = MySystem::new();
//!         let regions = system.get_memory_regions();
//!         
//!         assert!(regions.len() >= 2);
//!         assert!(regions.iter().any(|r| r.name == "RAM"));
//!         
//!         let ram = regions.iter().find(|r| r.name == "RAM").unwrap();
//!         assert_eq!(ram.start, 0x0000);
//!         assert_eq!(ram.end, 0x1FFF);
//!         assert!(ram.readable && ram.writable);
//!     }
//!
//!     #[test]
//!     fn test_cpu_state() {
//!         let system = MySystem::new();
//!         let state = system.get_cpu_state();
//!         
//!         assert!(state.registers.iter().any(|r| r.name == "PC"));
//!         assert!(state.registers.iter().any(|r| r.name == "A"));
//!         assert_eq!(state.flags.flags.len(), 7); // Adjust for your CPU
//!     }
//! }
//! ```

/// A memory region with a name and address range
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRegion {
    /// Region name (e.g., "ROM", "RAM", "VRAM", "PPU Registers")
    pub name: String,
    /// Start address (inclusive)
    pub start: u32,
    /// End address (inclusive)
    pub end: u32,
    /// Human-readable description
    pub description: String,
    /// Whether this region is readable
    pub readable: bool,
    /// Whether this region is writable
    pub writable: bool,
}

impl MemoryRegion {
    /// Create a new memory region
    pub fn new(
        name: impl Into<String>,
        start: u32,
        end: u32,
        description: impl Into<String>,
        readable: bool,
        writable: bool,
    ) -> Self {
        Self {
            name: name.into(),
            start,
            end,
            description: description.into(),
            readable,
            writable,
        }
    }

    /// Get the size of this region in bytes
    pub fn size(&self) -> u32 {
        self.end.saturating_sub(self.start).saturating_add(1)
    }

    /// Check if an address is within this region
    pub fn contains(&self, address: u32) -> bool {
        address >= self.start && address <= self.end
    }
}

/// A disassembled instruction
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisassembledInstruction {
    /// Program counter / address of instruction
    pub address: u32,
    /// Raw bytes of the instruction
    pub bytes: Vec<u8>,
    /// Disassembled mnemonic (e.g., "LDA #$10", "MOV AX, BX")
    pub mnemonic: String,
    /// Optional comment or annotation
    pub comment: Option<String>,
}

impl DisassembledInstruction {
    /// Create a new disassembled instruction
    pub fn new(address: u32, bytes: Vec<u8>, mnemonic: impl Into<String>) -> Self {
        Self {
            address,
            bytes,
            mnemonic: mnemonic.into(),
            comment: None,
        }
    }

    /// Add a comment to the instruction
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Get the length of this instruction in bytes
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Check if this instruction has zero length (should never happen)
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// CPU register value with name
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuRegister {
    /// Register name (e.g., "PC", "A", "X", "Y", "SP")
    pub name: String,
    /// Register value
    pub value: u32,
    /// Register width in bits (8, 16, 32, 64)
    pub width: u8,
}

impl CpuRegister {
    /// Create a new CPU register
    pub fn new(name: impl Into<String>, value: u32, width: u8) -> Self {
        Self {
            name: name.into(),
            value,
            width,
        }
    }

    /// Create an 8-bit register
    pub fn new_8bit(name: impl Into<String>, value: u8) -> Self {
        Self::new(name, value as u32, 8)
    }

    /// Create a 16-bit register
    pub fn new_16bit(name: impl Into<String>, value: u16) -> Self {
        Self::new(name, value as u32, 16)
    }

    /// Create a 32-bit register
    pub fn new_32bit(name: impl Into<String>, value: u32) -> Self {
        Self::new(name, value, 32)
    }
}

/// CPU flags/status register with individual flag states
#[derive(Debug, Clone)]
pub struct CpuFlags {
    /// Flag descriptions and their current states
    pub flags: Vec<(String, bool)>,
}

impl CpuFlags {
    /// Create a new CPU flags structure
    pub fn new() -> Self {
        Self { flags: Vec::new() }
    }

    /// Add a flag
    pub fn add_flag(&mut self, name: impl Into<String>, value: bool) {
        self.flags.push((name.into(), value));
    }
}

impl Default for CpuFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete CPU state snapshot
#[derive(Debug, Clone)]
pub struct CpuState {
    /// CPU registers
    pub registers: Vec<CpuRegister>,
    /// CPU flags
    pub flags: CpuFlags,
    /// Current program counter (for convenience)
    pub pc: u32,
}

impl CpuState {
    /// Create a new CPU state
    pub fn new(pc: u32) -> Self {
        Self {
            registers: Vec::new(),
            flags: CpuFlags::new(),
            pc,
        }
    }

    /// Add a register to the state
    pub fn add_register(&mut self, register: CpuRegister) {
        self.registers.push(register);
    }

    /// Add a flag to the state
    pub fn add_flag(&mut self, name: impl Into<String>, value: bool) {
        self.flags.add_flag(name, value);
    }
}

/// Execution trace entry
#[derive(Debug, Clone)]
pub struct ExecutionTrace {
    /// Instruction that was executed
    pub instruction: DisassembledInstruction,
    /// CPU state after execution
    pub cpu_state: CpuState,
}

/// Debugger interface for system introspection
pub trait Debugger {
    /// Disassemble a single instruction at the given address
    /// Returns None if the address is invalid or cannot be disassembled
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction>;

    /// Disassemble multiple instructions starting at the given address
    /// Returns up to `count` instructions
    fn disassemble_range(&self, address: u32, count: usize) -> Vec<DisassembledInstruction> {
        let mut result = Vec::new();
        let mut current_address = address;

        for _ in 0..count {
            if let Some(instruction) = self.disassemble_instruction(current_address) {
                current_address += instruction.len() as u32;
                result.push(instruction);
            } else {
                break;
            }
        }

        result
    }

    /// Read memory at the given address
    /// Returns None if the address is invalid or not readable
    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>>;

    /// Get the list of memory regions
    fn get_memory_regions(&self) -> Vec<MemoryRegion>;

    /// Get the current CPU state
    fn get_cpu_state(&self) -> CpuState;

    /// Get execution history (if available)
    /// Returns the most recent executed instructions
    /// Default implementation returns empty vector (no history tracking)
    fn get_execution_history(&self) -> Vec<ExecutionTrace> {
        Vec::new()
    }

    /// Check if execution history is enabled
    fn has_execution_history(&self) -> bool {
        false
    }
}

/// Helper macro to implement standard execution history methods for systems with instruction tracers.
/// This eliminates boilerplate by delegating to the system's `instruction_tracer` field.
///
/// # Example
/// ```ignore
/// impl Debugger for MySystem {
///     // ... other methods ...
///     
///     impl_debugger_execution_history!();
/// }
/// ```
#[macro_export]
macro_rules! impl_debugger_execution_history {
    () => {
        fn get_execution_history(&self) -> Vec<$crate::debug::ExecutionTrace> {
            self.instruction_tracer.get_history()
        }

        fn has_execution_history(&self) -> bool {
            self.instruction_tracer.is_enabled()
        }
    };
}

/// Helper macro to implement standard instruction tracer helper methods.
/// This eliminates boilerplate for systems that have an `instruction_tracer` field.
///
/// Provides:
/// - `set_instruction_tracing(enabled: bool)` - Enable/disable tracing
/// - `get_instruction_tracer() -> &InstructionTracer` - Get reference to tracer
///
/// # Example
/// ```ignore
/// impl MySystem {
///     impl_instruction_tracer_methods!();
///     
///     // ... other methods ...
/// }
/// ```
#[macro_export]
macro_rules! impl_instruction_tracer_methods {
    () => {
        /// Enable or disable instruction tracing
        pub fn set_instruction_tracing(&mut self, enabled: bool) {
            self.instruction_tracer.set_enabled(enabled);
        }

        /// Get a reference to the instruction tracer
        pub fn get_instruction_tracer(&self) -> &$crate::instruction_tracer::InstructionTracer {
            &self.instruction_tracer
        }

        /// Get a mutable reference to the instruction tracer
        pub fn get_instruction_tracer_mut(
            &mut self,
        ) -> &mut $crate::instruction_tracer::InstructionTracer {
            &mut self.instruction_tracer
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_region() {
        let region = MemoryRegion::new("RAM", 0x0000, 0x07FF, "Internal RAM", true, true);
        assert_eq!(region.name, "RAM");
        assert_eq!(region.start, 0x0000);
        assert_eq!(region.end, 0x07FF);
        assert_eq!(region.size(), 0x0800);
        assert!(region.contains(0x0400));
        assert!(!region.contains(0x0800));
        assert!(region.readable);
        assert!(region.writable);
    }

    #[test]
    fn test_disassembled_instruction() {
        let instr = DisassembledInstruction::new(0x8000, vec![0xA9, 0x10], "LDA #$10");
        assert_eq!(instr.address, 0x8000);
        assert_eq!(instr.bytes, vec![0xA9, 0x10]);
        assert_eq!(instr.mnemonic, "LDA #$10");
        assert_eq!(instr.len(), 2);
        assert!(!instr.is_empty());

        let instr_with_comment = instr.with_comment("Load accumulator with 16");
        assert_eq!(
            instr_with_comment.comment,
            Some("Load accumulator with 16".to_string())
        );
    }

    #[test]
    fn test_cpu_register() {
        let reg8 = CpuRegister::new_8bit("A", 0x42);
        assert_eq!(reg8.name, "A");
        assert_eq!(reg8.value, 0x42);
        assert_eq!(reg8.width, 8);

        let reg16 = CpuRegister::new_16bit("PC", 0x8000);
        assert_eq!(reg16.name, "PC");
        assert_eq!(reg16.value, 0x8000);
        assert_eq!(reg16.width, 16);

        let reg32 = CpuRegister::new_32bit("EAX", 0x12345678);
        assert_eq!(reg32.name, "EAX");
        assert_eq!(reg32.value, 0x12345678);
        assert_eq!(reg32.width, 32);
    }

    #[test]
    fn test_cpu_flags() {
        let mut flags = CpuFlags::new();
        flags.add_flag("Z", true);
        flags.add_flag("N", false);
        flags.add_flag("C", true);

        assert_eq!(flags.flags.len(), 3);
        assert_eq!(flags.flags[0], ("Z".to_string(), true));
        assert_eq!(flags.flags[1], ("N".to_string(), false));
        assert_eq!(flags.flags[2], ("C".to_string(), true));
    }

    #[test]
    fn test_cpu_state() {
        let mut state = CpuState::new(0x8000);
        state.add_register(CpuRegister::new_8bit("A", 0x42));
        state.add_register(CpuRegister::new_8bit("X", 0x10));
        state.add_flag("Z", true);
        state.add_flag("N", false);

        assert_eq!(state.pc, 0x8000);
        assert_eq!(state.registers.len(), 2);
        assert_eq!(state.flags.flags.len(), 2);
    }
}
