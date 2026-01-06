//! Breakpoint management for debugging.
//!
//! This module provides a simple breakpoint system for pausing emulation
//! at specific program counter (PC) addresses or conditions.

use std::collections::HashSet;

/// Type of breakpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BreakpointType {
    /// Break when PC equals the specified address
    Execute,
    /// Break when memory at address is read
    Read,
    /// Break when memory at address is written
    Write,
}

/// A breakpoint with an address and type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Breakpoint {
    /// Address to break at
    pub address: u32,
    /// Type of breakpoint
    pub breakpoint_type: BreakpointType,
}

impl Breakpoint {
    /// Create a new execution breakpoint
    pub fn new_execute(address: u32) -> Self {
        Self {
            address,
            breakpoint_type: BreakpointType::Execute,
        }
    }

    /// Create a new read breakpoint
    pub fn new_read(address: u32) -> Self {
        Self {
            address,
            breakpoint_type: BreakpointType::Read,
        }
    }

    /// Create a new write breakpoint
    pub fn new_write(address: u32) -> Self {
        Self {
            address,
            breakpoint_type: BreakpointType::Write,
        }
    }
}

/// Breakpoint manager
#[derive(Debug)]
pub struct BreakpointManager {
    /// Set of active breakpoints
    breakpoints: HashSet<Breakpoint>,
    /// Whether breakpoints are enabled
    enabled: bool,
}

impl BreakpointManager {
    /// Create a new breakpoint manager
    pub fn new() -> Self {
        Self {
            breakpoints: HashSet::new(),
            enabled: true,
        }
    }

    /// Add a breakpoint
    pub fn add(&mut self, breakpoint: Breakpoint) {
        self.breakpoints.insert(breakpoint);
    }

    /// Add an execution breakpoint at the given address
    pub fn add_execute(&mut self, address: u32) {
        self.add(Breakpoint::new_execute(address));
    }

    /// Add a read breakpoint at the given address
    pub fn add_read(&mut self, address: u32) {
        self.add(Breakpoint::new_read(address));
    }

    /// Add a write breakpoint at the given address
    pub fn add_write(&mut self, address: u32) {
        self.add(Breakpoint::new_write(address));
    }

    /// Remove a breakpoint
    pub fn remove(&mut self, breakpoint: &Breakpoint) -> bool {
        self.breakpoints.remove(breakpoint)
    }

    /// Remove all execution breakpoints at the given address
    pub fn remove_execute(&mut self, address: u32) -> bool {
        self.remove(&Breakpoint::new_execute(address))
    }

    /// Remove all breakpoints
    pub fn clear(&mut self) {
        self.breakpoints.clear();
    }

    /// Check if a breakpoint should trigger for execution
    pub fn should_break_execute(&self, address: u32) -> bool {
        if !self.enabled {
            return false;
        }
        self.breakpoints.contains(&Breakpoint::new_execute(address))
    }

    /// Check if a breakpoint should trigger for memory read
    pub fn should_break_read(&self, address: u32) -> bool {
        if !self.enabled {
            return false;
        }
        self.breakpoints.contains(&Breakpoint::new_read(address))
    }

    /// Check if a breakpoint should trigger for memory write
    pub fn should_break_write(&self, address: u32) -> bool {
        if !self.enabled {
            return false;
        }
        self.breakpoints.contains(&Breakpoint::new_write(address))
    }

    /// Get all breakpoints
    pub fn get_all(&self) -> Vec<Breakpoint> {
        self.breakpoints.iter().cloned().collect()
    }

    /// Get execution breakpoints
    pub fn get_execute_breakpoints(&self) -> Vec<u32> {
        self.breakpoints
            .iter()
            .filter(|bp| bp.breakpoint_type == BreakpointType::Execute)
            .map(|bp| bp.address)
            .collect()
    }

    /// Enable or disable breakpoints
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if breakpoints are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the number of breakpoints
    pub fn count(&self) -> usize {
        self.breakpoints.len()
    }
}

impl Default for BreakpointManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_creation() {
        let bp = Breakpoint::new_execute(0x8000);
        assert_eq!(bp.address, 0x8000);
        assert_eq!(bp.breakpoint_type, BreakpointType::Execute);

        let bp = Breakpoint::new_read(0x2000);
        assert_eq!(bp.address, 0x2000);
        assert_eq!(bp.breakpoint_type, BreakpointType::Read);

        let bp = Breakpoint::new_write(0x0200);
        assert_eq!(bp.address, 0x0200);
        assert_eq!(bp.breakpoint_type, BreakpointType::Write);
    }

    #[test]
    fn test_breakpoint_manager_add_remove() {
        let mut mgr = BreakpointManager::new();
        assert_eq!(mgr.count(), 0);

        mgr.add_execute(0x8000);
        assert_eq!(mgr.count(), 1);
        assert!(mgr.should_break_execute(0x8000));
        assert!(!mgr.should_break_execute(0x8001));

        mgr.add_read(0x2000);
        assert_eq!(mgr.count(), 2);
        assert!(mgr.should_break_read(0x2000));
        assert!(!mgr.should_break_read(0x2001));

        assert!(mgr.remove_execute(0x8000));
        assert_eq!(mgr.count(), 1);
        assert!(!mgr.should_break_execute(0x8000));

        mgr.clear();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn test_breakpoint_manager_enable_disable() {
        let mut mgr = BreakpointManager::new();
        mgr.add_execute(0x8000);
        assert!(mgr.should_break_execute(0x8000));

        mgr.set_enabled(false);
        assert!(!mgr.should_break_execute(0x8000));

        mgr.set_enabled(true);
        assert!(mgr.should_break_execute(0x8000));
    }

    #[test]
    fn test_breakpoint_manager_get_all() {
        let mut mgr = BreakpointManager::new();
        mgr.add_execute(0x8000);
        mgr.add_execute(0x8010);
        mgr.add_read(0x2000);

        let all = mgr.get_all();
        assert_eq!(all.len(), 3);

        let exec_bps = mgr.get_execute_breakpoints();
        assert_eq!(exec_bps.len(), 2);
        assert!(exec_bps.contains(&0x8000));
        assert!(exec_bps.contains(&0x8010));
    }

    #[test]
    fn test_breakpoint_manager_duplicate() {
        let mut mgr = BreakpointManager::new();
        mgr.add_execute(0x8000);
        mgr.add_execute(0x8000); // Duplicate
        assert_eq!(mgr.count(), 1); // Should only have one
    }
}
