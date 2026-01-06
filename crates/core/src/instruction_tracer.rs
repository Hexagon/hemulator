//! Instruction execution tracer for debugging.
//!
//! This module provides a circular buffer for tracking executed instructions,
//! useful for debugging emulated systems without restarting repeatedly.

use crate::debug::{CpuState, DisassembledInstruction, ExecutionTrace};
use std::collections::VecDeque;

/// Configuration for the instruction tracer
#[derive(Debug, Clone)]
pub struct TracerConfig {
    /// Maximum number of instructions to keep in history
    pub max_history: usize,
    /// Whether to enable tracing (can be toggled at runtime)
    pub enabled: bool,
}

impl Default for TracerConfig {
    fn default() -> Self {
        Self {
            max_history: 10000, // Last 10,000 instructions
            enabled: false,
        }
    }
}

/// Instruction tracer with circular buffer
#[derive(Debug)]
pub struct InstructionTracer {
    /// Configuration
    config: TracerConfig,
    /// Circular buffer of execution traces
    history: VecDeque<ExecutionTrace>,
    /// Total number of instructions traced (wraps at u64::MAX)
    total_traced: u64,
}

impl InstructionTracer {
    /// Create a new instruction tracer with default configuration
    pub fn new() -> Self {
        Self::with_config(TracerConfig::default())
    }

    /// Create a new instruction tracer with custom configuration
    pub fn with_config(config: TracerConfig) -> Self {
        Self {
            history: VecDeque::with_capacity(config.max_history),
            config,
            total_traced: 0,
        }
    }

    /// Enable or disable tracing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Check if tracing is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Record an executed instruction
    pub fn trace(&mut self, instruction: DisassembledInstruction, cpu_state: CpuState) {
        if !self.config.enabled {
            return;
        }

        // Add to history
        self.history.push_back(ExecutionTrace {
            instruction,
            cpu_state,
        });

        // Remove oldest if we exceed capacity
        if self.history.len() > self.config.max_history {
            self.history.pop_front();
        }

        self.total_traced = self.total_traced.wrapping_add(1);
    }

    /// Get the execution history (most recent first)
    pub fn get_history(&self) -> Vec<ExecutionTrace> {
        self.history.iter().rev().cloned().collect()
    }

    /// Get the last N instructions from history
    pub fn get_last_n(&self, n: usize) -> Vec<ExecutionTrace> {
        self.history.iter().rev().take(n).cloned().collect()
    }

    /// Clear the execution history
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// Get the total number of instructions traced
    pub fn total_traced(&self) -> u64 {
        self.total_traced
    }

    /// Get the current history size
    pub fn history_size(&self) -> usize {
        self.history.len()
    }

    /// Dump the execution trace to a file
    pub fn dump_to_file(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(path)?;

        writeln!(file, "===============================================")?;
        writeln!(file, "    INSTRUCTION EXECUTION TRACE")?;
        writeln!(file, "===============================================")?;
        writeln!(file)?;
        writeln!(
            file,
            "Timestamp: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )?;
        writeln!(file, "Total Instructions Traced: {}", self.total_traced)?;
        writeln!(file, "History Size: {} instructions", self.history.len())?;
        writeln!(file)?;
        writeln!(file, "===============================================")?;
        writeln!(file, "    EXECUTION HISTORY (newest first)")?;
        writeln!(file, "===============================================")?;
        writeln!(file)?;

        // Dump instructions in reverse order (newest first)
        for (i, trace) in self.history.iter().rev().enumerate() {
            let instr = &trace.instruction;
            let state = &trace.cpu_state;

            // Format: index, address, bytes, mnemonic, PC after execution
            let bytes_str: String = instr
                .bytes
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");

            writeln!(
                file,
                "{:6}: ${:04X}  {:12}  {:20}  ; PC=${:04X}",
                i, instr.address, bytes_str, instr.mnemonic, state.pc
            )?;

            // Add comment if present
            if let Some(ref comment) = instr.comment {
                writeln!(file, "        ; {}", comment)?;
            }
        }

        writeln!(file)?;
        writeln!(file, "===============================================")?;
        writeln!(file, "    END OF TRACE")?;
        writeln!(file, "===============================================")?;

        Ok(())
    }
}

impl Default for InstructionTracer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug::CpuRegister;

    #[test]
    fn test_tracer_basic() {
        let mut tracer = InstructionTracer::new();
        assert!(!tracer.is_enabled());
        assert_eq!(tracer.history_size(), 0);

        tracer.set_enabled(true);
        assert!(tracer.is_enabled());
    }

    #[test]
    fn test_tracer_recording() {
        let mut tracer = InstructionTracer::new();
        tracer.set_enabled(true);

        let instr = DisassembledInstruction::new(0x8000, vec![0xA9, 0x42], "LDA #$42");
        let mut state = CpuState::new(0x8002);
        state.add_register(CpuRegister::new_8bit("A", 0x42));

        tracer.trace(instr, state);

        assert_eq!(tracer.history_size(), 1);
        assert_eq!(tracer.total_traced(), 1);

        let history = tracer.get_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].instruction.address, 0x8000);
    }

    #[test]
    fn test_tracer_circular_buffer() {
        let config = TracerConfig {
            max_history: 3,
            enabled: true,
        };
        let mut tracer = InstructionTracer::with_config(config);

        // Add 5 instructions
        for i in 0..5 {
            let addr = 0x8000 + i * 2;
            let instr = DisassembledInstruction::new(addr, vec![0xEA], "NOP");
            let state = CpuState::new(addr + 1);
            tracer.trace(instr, state);
        }

        // Should only keep last 3
        assert_eq!(tracer.history_size(), 3);
        assert_eq!(tracer.total_traced(), 5);

        let history = tracer.get_history();
        // Newest first
        assert_eq!(history[0].instruction.address, 0x8008);
        assert_eq!(history[1].instruction.address, 0x8006);
        assert_eq!(history[2].instruction.address, 0x8004);
    }

    #[test]
    fn test_tracer_get_last_n() {
        let mut tracer = InstructionTracer::new();
        tracer.set_enabled(true);

        // Add 10 instructions
        for i in 0..10 {
            let addr = 0x8000 + i * 2;
            let instr = DisassembledInstruction::new(addr, vec![0xEA], "NOP");
            let state = CpuState::new(addr + 1);
            tracer.trace(instr, state);
        }

        let last_3 = tracer.get_last_n(3);
        assert_eq!(last_3.len(), 3);
        assert_eq!(last_3[0].instruction.address, 0x8012); // Newest
        assert_eq!(last_3[2].instruction.address, 0x800E);
    }

    #[test]
    fn test_tracer_disabled() {
        let mut tracer = InstructionTracer::new();
        // Tracing is disabled by default

        let instr = DisassembledInstruction::new(0x8000, vec![0xEA], "NOP");
        let state = CpuState::new(0x8001);
        tracer.trace(instr, state);

        // Should not have recorded anything
        assert_eq!(tracer.history_size(), 0);
        assert_eq!(tracer.total_traced(), 0);
    }

    #[test]
    fn test_tracer_clear() {
        let mut tracer = InstructionTracer::new();
        tracer.set_enabled(true);

        // Add some instructions
        for i in 0..5 {
            let addr = 0x8000 + i * 2;
            let instr = DisassembledInstruction::new(addr, vec![0xEA], "NOP");
            let state = CpuState::new(addr + 1);
            tracer.trace(instr, state);
        }

        assert_eq!(tracer.history_size(), 5);

        tracer.clear();
        assert_eq!(tracer.history_size(), 0);
        // Total traced should not be reset
        assert_eq!(tracer.total_traced(), 5);
    }
}
