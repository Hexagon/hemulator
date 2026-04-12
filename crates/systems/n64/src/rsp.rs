//! RSP (Reality Signal Processor) - Coprocessor for Nintendo 64
//!
//! The RSP is part of the RCP (Reality Co-Processor) and handles:
//! - Geometry processing (vertex transforms, lighting)
//! - Audio processing (audio mixing, effects)
//! - Display list generation for RDP
//!
//! # Architecture Overview
//!
//! The RSP consists of:
//! - **4KB DMEM**: Data memory for working storage
//! - **4KB IMEM**: Instruction memory for microcode
//! - **Scalar Unit (SU)**: MIPS-like CPU for control flow
//! - **Vector Unit (VU)**: 8-way SIMD processor for parallel operations
//! - **32 Vector Registers**: Each 128 bits (16 bytes, 8 elements of 16 bits)
//!
//! ## Microcode
//!
//! The RSP executes microcode programs loaded by games:
//! - **Graphics microcode** (gspFast3D, gspF3DEX, gspF3DEX2, etc.): Vertex processing
//! - **Audio microcode**: Sound synthesis and mixing
//! - **Custom microcode**: Game-specific processing
//!
//! # Implementation Status
//!
//! This implementation uses **High-Level Emulation (HLE)**:
//! - Memory (DMEM/IMEM) allocated and accessible
//! - Register interface for DMA and control
//! - HLE for common graphics microcodes (F3DEX/F3DEX2)
//! - Automatic microcode detection when loaded into IMEM
//! - Task execution via HLE instead of instruction-level emulation
//!
//! Low-level instruction execution would require:
//! - MIPS R4000-based scalar unit interpreter/JIT
//! - Vector unit emulation with 32x128-bit registers
//! - Full microcode execution at instruction level

/// RSP register addresses (relative to 0x04040000)
#[allow(dead_code)]
const SP_MEM_ADDR: u32 = 0x00; // SP memory address
#[allow(dead_code)]
const SP_DRAM_ADDR: u32 = 0x04; // RDRAM address for DMA
#[allow(dead_code)]
const SP_RD_LEN: u32 = 0x08; // DMA length (read from RDRAM)
#[allow(dead_code)]
const SP_WR_LEN: u32 = 0x0C; // DMA length (write to RDRAM)
#[allow(dead_code)]
const SP_STATUS: u32 = 0x10; // Status register
#[allow(dead_code)]
const SP_DMA_FULL: u32 = 0x14; // DMA full
#[allow(dead_code)]
const SP_DMA_BUSY: u32 = 0x18; // DMA busy
#[allow(dead_code)]
const SP_SEMAPHORE: u32 = 0x1C; // Semaphore

/// RSP status register bits
#[allow(dead_code)]
const SP_STATUS_HALT: u32 = 0x001; // RSP halted
#[allow(dead_code)]
const SP_STATUS_BROKE: u32 = 0x002; // RSP break
#[allow(dead_code)]
const SP_STATUS_DMA_BUSY: u32 = 0x004; // DMA in progress
#[allow(dead_code)]
const SP_STATUS_DMA_FULL: u32 = 0x008; // DMA queue full
#[allow(dead_code)]
const SP_STATUS_IO_FULL: u32 = 0x010; // I/O full
#[allow(dead_code)]
const SP_STATUS_SSTEP: u32 = 0x020; // Single step mode
#[allow(dead_code)]
const SP_STATUS_INTR_BREAK: u32 = 0x040; // Interrupt on break
const SP_STATUS_SIG0: u32 = 0x080; // Signal 0
const SP_STATUS_SIG1: u32 = 0x100; // Signal 1
const SP_STATUS_SIG2: u32 = 0x200; // Signal 2
const SP_STATUS_SIG3: u32 = 0x400; // Signal 3
const SP_STATUS_SIG4: u32 = 0x800; // Signal 4
const SP_STATUS_SIG5: u32 = 0x1000; // Signal 5
const SP_STATUS_SIG6: u32 = 0x2000; // Signal 6
const SP_STATUS_SIG7: u32 = 0x4000; // Signal 7

use super::rdp::Rdp;
use super::rsp_hle::RspHle;
use std::cell::Cell;

/// RSP (Reality Signal Processor) state
pub struct Rsp {
    /// 4KB DMEM (Data Memory)
    dmem: [u8; 4096],

    /// 4KB IMEM (Instruction Memory)
    imem: [u8; 4096],

    /// Program counter
    pub pc: u32,

    /// RSP registers
    sp_mem_addr: u32,
    sp_dram_addr: u32,
    sp_rd_len: u32,
    sp_wr_len: u32,
    sp_status: u32,
    sp_dma_full: u32,
    sp_dma_busy: u32,
    /// Semaphore uses Cell for interior mutability (read clears it)
    sp_semaphore: Cell<u32>,

    /// High-level emulation state
    hle: RspHle,

    /// RDRAM address of the last task structure DMA'd to DMEM[FC0].
    /// Used to clear the task type field after audio HLE completion.
    last_task_dram_addr: u32,
}

impl Rsp {
    /// Create a new RSP
    pub fn new() -> Self {
        Self {
            dmem: [0; 4096],
            imem: [0; 4096],
            pc: 0,
            sp_mem_addr: 0,
            sp_dram_addr: 0,
            sp_rd_len: 0,
            sp_wr_len: 0,
            sp_status: SP_STATUS_HALT, // Start halted
            sp_dma_full: 0,
            sp_dma_busy: 0,
            sp_semaphore: Cell::new(0),
            hle: RspHle::new(),
            last_task_dram_addr: 0,
        }
    }

    /// Reset RSP to initial state
    #[allow(dead_code)] // Used in tests and will be needed when RSP execution is implemented
    pub fn reset(&mut self) {
        self.dmem.fill(0);
        self.imem.fill(0);
        self.pc = 0;
        self.sp_mem_addr = 0;
        self.sp_dram_addr = 0;
        self.sp_rd_len = 0;
        self.sp_wr_len = 0;
        self.sp_status = SP_STATUS_HALT;
        self.sp_dma_full = 0;
        self.sp_dma_busy = 0;
        self.sp_semaphore.set(0);
        self.hle = RspHle::new();
    }

    /// Read from DMEM
    pub fn read_dmem(&self, offset: u32) -> u8 {
        let addr = (offset & 0xFFF) as usize;
        self.dmem[addr]
    }

    /// Write to DMEM
    pub fn write_dmem(&mut self, offset: u32, value: u8) {
        let addr = (offset & 0xFFF) as usize;
        self.dmem[addr] = value;
    }

    /// Read from IMEM
    pub fn read_imem(&self, offset: u32) -> u8 {
        let addr = (offset & 0xFFF) as usize;
        self.imem[addr]
    }

    /// Write to IMEM
    pub fn write_imem(&mut self, offset: u32, value: u8) {
        let addr = (offset & 0xFFF) as usize;
        self.imem[addr] = value;

        // Detect microcode when IMEM is written
        // (Simplified: only detect after first write, could optimize)
        if addr == 0 {
            self.hle.detect_microcode(&self.imem);
        }
    }

    /// Read from RSP register
    pub fn read_register(&self, offset: u32) -> u32 {
        match offset {
            SP_MEM_ADDR => self.sp_mem_addr,
            SP_DRAM_ADDR => self.sp_dram_addr,
            SP_RD_LEN => self.sp_rd_len,
            SP_WR_LEN => self.sp_wr_len,
            SP_STATUS => self.sp_status,
            SP_DMA_FULL => self.sp_dma_full,
            SP_DMA_BUSY => self.sp_dma_busy,
            SP_SEMAPHORE => {
                // Read returns current value, then sets to 1 (locked)
                // First read after write/reset: returns 0 (acquired)
                // Subsequent reads: returns 1 (busy, not acquired)
                let val = self.sp_semaphore.get();
                self.sp_semaphore.set(1);
                val
            }
            _ => 0,
        }
    }

    /// Write to RSP register
    pub fn write_register(&mut self, offset: u32, value: u32, rdram: &mut [u8]) {
        match offset {
            SP_MEM_ADDR => {
                self.sp_mem_addr = value & 0x1FFF; // 13-bit address
            }
            SP_DRAM_ADDR => {
                self.sp_dram_addr = value & 0x00FFFFFF; // 24-bit address
            }
            SP_RD_LEN => {
                // DMA read from RDRAM to RSP memory (DMEM or IMEM)
                // Bits 0-11: length per line minus 1
                // Bits 12-19: count (number of lines minus 1)
                // Bits 20-31: skip (DRAM skip between lines)
                self.sp_rd_len = value;
                use emu_core::logging::{log, LogCategory, LogLevel};
                log(LogCategory::PPU, LogLevel::Info, || {
                    format!(
                        "RSP: DMA read - DRAM:0x{:08X} -> RSP:0x{:04X}, len:{}",
                        self.sp_dram_addr,
                        self.sp_mem_addr,
                        (self.sp_rd_len & 0xFFF) + 1
                    )
                });
                self.dma_read(rdram);
            }
            SP_WR_LEN => {
                // DMA write from RSP memory to RDRAM
                // Bits 0-11: length per line minus 1
                // Bits 12-19: count (number of lines minus 1)
                // Bits 20-31: skip (DRAM skip between lines)
                self.sp_wr_len = value;
                self.dma_write(rdram);
            }
            SP_STATUS => {
                // Status register write (control bits)
                use emu_core::logging::{log, LogCategory, LogLevel};
                log(LogCategory::PPU, LogLevel::Info, || {
                    format!("RSP: SP_STATUS write 0x{:08X}", value)
                });

                // Bit 0: Clear halt
                if value & 0x0001 != 0 {
                    self.sp_status &= !SP_STATUS_HALT;
                    log(LogCategory::PPU, LogLevel::Info, || {
                        "RSP: Cleared HALT flag (RSP starting)".to_string()
                    });
                }
                // Bit 1: Set halt
                if value & 0x0002 != 0 {
                    self.sp_status |= SP_STATUS_HALT;
                    log(LogCategory::PPU, LogLevel::Info, || {
                        "RSP: Set HALT flag (RSP stopping)".to_string()
                    });
                }
                // Bit 2: Clear broke
                if value & 0x0004 != 0 {
                    self.sp_status &= !SP_STATUS_BROKE;
                }
                // Bit 3: Clear interrupt
                if value & 0x0008 != 0 {
                    // Interrupt clearing would be handled by MI
                }
                // Bit 4: Set interrupt
                if value & 0x0010 != 0 {
                    // Interrupt setting would be handled by MI
                }
                // Bit 5: Clear single step
                if value & 0x0020 != 0 {
                    self.sp_status &= !SP_STATUS_SSTEP;
                }
                // Bit 6: Set single step
                if value & 0x0040 != 0 {
                    self.sp_status |= SP_STATUS_SSTEP;
                }
                // Bit 7: Clear interrupt on break
                if value & 0x0080 != 0 {
                    self.sp_status &= !SP_STATUS_INTR_BREAK;
                }
                // Bit 8: Set interrupt on break
                if value & 0x0100 != 0 {
                    self.sp_status |= SP_STATUS_INTR_BREAK;
                }
                // Bits 9-10: Clear/Set signal 0
                if value & 0x0200 != 0 {
                    self.sp_status &= !SP_STATUS_SIG0;
                }
                if value & 0x0400 != 0 {
                    self.sp_status |= SP_STATUS_SIG0;
                }
                // Bits 11-12: Clear/Set signal 1
                if value & 0x0800 != 0 {
                    self.sp_status &= !SP_STATUS_SIG1;
                }
                if value & 0x1000 != 0 {
                    self.sp_status |= SP_STATUS_SIG1;
                }
                // Bits 13-14: Clear/Set signal 2
                if value & 0x2000 != 0 {
                    self.sp_status &= !SP_STATUS_SIG2;
                }
                if value & 0x4000 != 0 {
                    self.sp_status |= SP_STATUS_SIG2;
                }
                // Bits 15-16: Clear/Set signal 3
                if value & 0x8000 != 0 {
                    self.sp_status &= !SP_STATUS_SIG3;
                }
                if value & 0x10000 != 0 {
                    self.sp_status |= SP_STATUS_SIG3;
                }
                // Bits 17-18: Clear/Set signal 4
                if value & 0x20000 != 0 {
                    self.sp_status &= !SP_STATUS_SIG4;
                }
                if value & 0x40000 != 0 {
                    self.sp_status |= SP_STATUS_SIG4;
                }
                // Bits 19-20: Clear/Set signal 5
                if value & 0x80000 != 0 {
                    self.sp_status &= !SP_STATUS_SIG5;
                }
                if value & 0x100000 != 0 {
                    self.sp_status |= SP_STATUS_SIG5;
                }
                // Bits 21-22: Clear/Set signal 6
                if value & 0x200000 != 0 {
                    self.sp_status &= !SP_STATUS_SIG6;
                }
                if value & 0x400000 != 0 {
                    self.sp_status |= SP_STATUS_SIG6;
                }
                // Bits 23-24: Clear/Set signal 7
                if value & 0x800000 != 0 {
                    self.sp_status &= !SP_STATUS_SIG7;
                }
                if value & 0x1000000 != 0 {
                    self.sp_status |= SP_STATUS_SIG7;
                }
            }
            SP_SEMAPHORE => {
                // Writing any value to semaphore releases it (sets to 0)
                self.sp_semaphore.set(0);
            }
            _ => {}
        }
    }

    /// DMA read from RDRAM to RSP memory
    /// Supports multi-line transfers with count and skip fields
    fn dma_read(&mut self, rdram: &[u8]) {
        let length = (self.sp_rd_len & 0xFFF) + 1; // bytes per line
        let count = ((self.sp_rd_len >> 12) & 0xFF) + 1; // number of lines
        let skip = (self.sp_rd_len >> 20) & 0xFFF; // DRAM skip between lines
        let mut dram_addr = (self.sp_dram_addr & 0x00FFFFFF) as usize;
        let mut mem_addr = (self.sp_mem_addr & 0x1FFF) as usize;
        let is_imem = (self.sp_mem_addr & 0x1000) != 0;

        // Track if this DMA loads the task structure area (DMEM[FC0..FFF])
        if !is_imem && (mem_addr & 0xFFF) == 0xFC0 {
            self.last_task_dram_addr = self.sp_dram_addr & 0x00FFFFFF;
        }

        for _line in 0..count {
            for i in 0..length as usize {
                if dram_addr + i < rdram.len() {
                    let value = rdram[dram_addr + i];
                    let dest_addr = (mem_addr + i) & 0xFFF;

                    if is_imem {
                        self.imem[dest_addr] = value;
                    } else {
                        self.dmem[dest_addr] = value;
                    }
                }
            }
            // Advance pointers for next line
            dram_addr += length as usize + skip as usize;
            mem_addr += length as usize;
        }

        // If we just loaded IMEM, detect microcode
        if is_imem {
            use emu_core::logging::{log, LogCategory, LogLevel};
            self.hle.detect_microcode(&self.imem);
            log(LogCategory::PPU, LogLevel::Info, || {
                format!("RSP: Microcode detected: {:?}", self.hle.microcode())
            });
        }
    }

    /// DMA write from RSP memory to RDRAM
    /// Supports multi-line transfers with count and skip fields
    fn dma_write(&mut self, rdram: &mut [u8]) {
        let length = (self.sp_wr_len & 0xFFF) + 1; // bytes per line
        let count = ((self.sp_wr_len >> 12) & 0xFF) + 1; // number of lines
        let skip = (self.sp_wr_len >> 20) & 0xFFF; // DRAM skip between lines
        let mut dram_addr = (self.sp_dram_addr & 0x00FFFFFF) as usize;
        let mut mem_addr = (self.sp_mem_addr & 0x1FFF) as usize;
        let is_imem = (self.sp_mem_addr & 0x1000) != 0;

        for _line in 0..count {
            for i in 0..length as usize {
                let src_addr = (mem_addr + i) & 0xFFF;
                let value = if is_imem {
                    self.imem[src_addr]
                } else {
                    self.dmem[src_addr]
                };

                if dram_addr + i < rdram.len() {
                    rdram[dram_addr + i] = value;
                }
            }
            // Advance pointers for next line
            dram_addr += length as usize + skip as usize;
            mem_addr += length as usize;
        }
    }

    /// Execute RSP task via HLE
    /// Called when RSP is un-halted by writing to SP_STATUS
    /// Returns (cycles, should_interrupt)
    pub fn execute_task(&mut self, rdram: &mut [u8], rdp: &mut Rdp) -> (u32, bool) {
        use emu_core::logging::{log, LogCategory, LogLevel};

        // Check if RSP is halted
        if self.sp_status & SP_STATUS_HALT != 0 {
            log(LogCategory::PPU, LogLevel::Debug, || {
                "RSP: execute_task() called but RSP is halted".to_string()
            });
            return (0, false);
        }

        log(LogCategory::PPU, LogLevel::Info, || {
            format!(
                "RSP: Executing task (microcode: {:?})",
                self.hle.microcode()
            )
        });

        // Execute HLE task
        let cycles = self.hle.execute_task(&self.dmem, rdram, rdp);

        log(LogCategory::PPU, LogLevel::Info, || {
            format!("RSP: Task complete ({} cycles)", cycles)
        });

        // Set broke flag, halt, and signal 1 (task done) after task completion.
        // Signal 1 (SIG1 = bit 8) tells the N64 OS interrupt handler that the RSP
        // task completed normally (as opposed to yielding). Without SIG1, the handler
        // ignores the SP interrupt and never notifies the scheduler of task completion.
        // Real RSP microcode (F3DEX/F3DEX2) explicitly sets this signal before halting.
        self.sp_status |= SP_STATUS_BROKE | SP_STATUS_HALT | SP_STATUS_SIG1;

        // Check if interrupt on break is enabled
        let should_interrupt = (self.sp_status & SP_STATUS_INTR_BREAK) != 0;

        (cycles, should_interrupt)
    }

    /// Get current SP_STATUS register value
    #[allow(dead_code)]
    pub fn get_sp_status(&self) -> u32 {
        self.sp_status
    }

    /// Get current microcode type (for debugging/monitoring)
    #[allow(dead_code)]
    pub fn microcode(&self) -> super::rsp_hle::MicrocodeType {
        self.hle.microcode()
    }

    /// Mark the RSP as having finished its task (set BROKE+HALT).
    /// Called by the bus when the deferred RSP interrupt timer expires,
    /// simulating the moment the RSP finishes execution on real hardware.
    #[allow(dead_code)]
    pub fn set_task_complete(&mut self) {
        self.sp_status |= SP_STATUS_BROKE | SP_STATUS_HALT;
    }

    /// Get the RDRAM address of the last task structure loaded via DMA.
    #[allow(dead_code)]
    pub fn get_last_task_dram_addr(&self) -> u32 {
        self.last_task_dram_addr
    }

    /// Get current microcode type
    pub fn microcode_type(&self) -> super::rsp_hle::MicrocodeType {
        self.hle.microcode()
    }

    /// Get vertex count in RSP vertex buffer
    pub fn vertex_count(&self) -> usize {
        self.hle.vertex_count()
    }

    /// Debug: get and reset zero matrix count
    #[allow(dead_code)]
    pub fn take_zero_mtx_count(&mut self) -> u32 {
        let c = self.hle.zero_mtx_count;
        self.hle.zero_mtx_count = 0;
        c
    }
}

impl Default for Rsp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsp_creation() {
        let rsp = Rsp::new();
        assert_eq!(rsp.pc, 0);
        assert_eq!(rsp.sp_status & SP_STATUS_HALT, SP_STATUS_HALT);
    }

    #[test]
    fn test_rsp_reset() {
        let mut rsp = Rsp::new();
        rsp.pc = 0x100;
        rsp.sp_status = 0;

        rsp.reset();

        assert_eq!(rsp.pc, 0);
        assert_eq!(rsp.sp_status & SP_STATUS_HALT, SP_STATUS_HALT);
    }

    #[test]
    fn test_rsp_dmem_access() {
        let mut rsp = Rsp::new();

        rsp.write_dmem(0x100, 0x42);
        assert_eq!(rsp.read_dmem(0x100), 0x42);

        // Test wrapping
        rsp.write_dmem(0x1100, 0x55); // Should wrap to 0x100
        assert_eq!(rsp.read_dmem(0x100), 0x55);
    }

    #[test]
    fn test_rsp_imem_access() {
        let mut rsp = Rsp::new();

        rsp.write_imem(0x200, 0x33);
        assert_eq!(rsp.read_imem(0x200), 0x33);
    }

    #[test]
    fn test_rsp_dma_read() {
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 1024];

        // Write test data to RDRAM
        rdram[0x100] = 0x11;
        rdram[0x101] = 0x22;
        rdram[0x102] = 0x33;
        rdram[0x103] = 0x44;

        // Set up DMA: copy 4 bytes from RDRAM 0x100 to DMEM 0x200
        rsp.sp_dram_addr = 0x100;
        rsp.sp_mem_addr = 0x200; // DMEM (bit 12 clear)
        rsp.sp_rd_len = 3; // length - 1
        rsp.dma_read(&rdram);

        // Verify data was copied
        assert_eq!(rsp.read_dmem(0x200), 0x11);
        assert_eq!(rsp.read_dmem(0x201), 0x22);
        assert_eq!(rsp.read_dmem(0x202), 0x33);
        assert_eq!(rsp.read_dmem(0x203), 0x44);
    }

    #[test]
    fn test_rsp_dma_write() {
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 4096]; // Increased size to accommodate test

        // Write test data to DMEM
        rsp.write_dmem(0x300, 0xAA);
        rsp.write_dmem(0x301, 0xBB);
        rsp.write_dmem(0x302, 0xCC);
        rsp.write_dmem(0x303, 0xDD);

        // Set up DMA: copy 4 bytes from DMEM 0x300 to RDRAM 0x500
        rsp.sp_dram_addr = 0x500;
        rsp.sp_mem_addr = 0x300; // DMEM
        rsp.sp_wr_len = 3; // length - 1
        rsp.dma_write(&mut rdram);

        // Verify data was copied
        assert_eq!(rdram[0x500], 0xAA);
        assert_eq!(rdram[0x501], 0xBB);
        assert_eq!(rdram[0x502], 0xCC);
        assert_eq!(rdram[0x503], 0xDD);
    }

    #[test]
    fn test_rsp_status_halt_control() {
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 1024];

        // RSP should start halted
        assert_eq!(rsp.sp_status & SP_STATUS_HALT, SP_STATUS_HALT);

        // Clear halt
        rsp.write_register(SP_STATUS, 0x0001, &mut rdram);
        assert_eq!(rsp.sp_status & SP_STATUS_HALT, 0);

        // Set halt
        rsp.write_register(SP_STATUS, 0x0002, &mut rdram);
        assert_eq!(rsp.sp_status & SP_STATUS_HALT, SP_STATUS_HALT);
    }

    #[test]
    fn test_rsp_dma_to_imem() {
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 1024];

        // Write test data to RDRAM
        rdram[0x100] = 0x12;
        rdram[0x101] = 0x34;

        // Set up DMA to IMEM (bit 12 set in mem_addr)
        rsp.sp_dram_addr = 0x100;
        rsp.sp_mem_addr = 0x1000; // IMEM (bit 12 set)
        rsp.sp_rd_len = 1; // 2 bytes
        rsp.dma_read(&rdram);

        // Verify data was copied to IMEM
        assert_eq!(rsp.read_imem(0x000), 0x12);
        assert_eq!(rsp.read_imem(0x001), 0x34);
    }

    #[test]
    fn test_rsp_semaphore_behavior() {
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 1024];

        // Initially semaphore is 0; first read acquires it (returns 0, sets to 1)
        assert_eq!(rsp.read_register(SP_SEMAPHORE), 0);

        // Second read returns 1 (already locked) and stays locked
        assert_eq!(rsp.read_register(SP_SEMAPHORE), 1);

        // Third read still returns 1 (locked)
        assert_eq!(rsp.read_register(SP_SEMAPHORE), 1);

        // Write releases (sets to 0)
        rsp.write_register(SP_SEMAPHORE, 0, &mut rdram);

        // Read after release: returns 0 (acquired), sets to 1
        assert_eq!(rsp.read_register(SP_SEMAPHORE), 0);

        // Now locked again
        assert_eq!(rsp.read_register(SP_SEMAPHORE), 1);
    }

    #[test]
    fn test_rsp_signal_bits() {
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 1024];

        // Initially all signal bits should be 0
        let status = rsp.read_register(SP_STATUS);
        assert_eq!(status & SP_STATUS_SIG0, 0);
        assert_eq!(status & SP_STATUS_SIG1, 0);

        // Set SIG0 (bit 10 = 0x0400)
        rsp.write_register(SP_STATUS, 0x0400, &mut rdram);
        let status = rsp.read_register(SP_STATUS);
        assert_eq!(status & SP_STATUS_SIG0, SP_STATUS_SIG0);

        // Clear SIG0 (bit 9 = 0x0200)
        rsp.write_register(SP_STATUS, 0x0200, &mut rdram);
        let status = rsp.read_register(SP_STATUS);
        assert_eq!(status & SP_STATUS_SIG0, 0);

        // Set SIG1 (bit 12 = 0x1000)
        rsp.write_register(SP_STATUS, 0x1000, &mut rdram);
        let status = rsp.read_register(SP_STATUS);
        assert_eq!(status & SP_STATUS_SIG1, SP_STATUS_SIG1);

        // Clear SIG1 (bit 11 = 0x0800)
        rsp.write_register(SP_STATUS, 0x0800, &mut rdram);
        let status = rsp.read_register(SP_STATUS);
        assert_eq!(status & SP_STATUS_SIG1, 0);

        // Test setting multiple signals at once
        rsp.write_register(SP_STATUS, 0x0400 | 0x1000, &mut rdram); // Set SIG0 and SIG1
        let status = rsp.read_register(SP_STATUS);
        assert_eq!(status & SP_STATUS_SIG0, SP_STATUS_SIG0);
        assert_eq!(status & SP_STATUS_SIG1, SP_STATUS_SIG1);

        // Test clearing multiple signals at once
        rsp.write_register(SP_STATUS, 0x0200 | 0x0800, &mut rdram); // Clear SIG0 and SIG1
        let status = rsp.read_register(SP_STATUS);
        assert_eq!(status & SP_STATUS_SIG0, 0);
        assert_eq!(status & SP_STATUS_SIG1, 0);
    }

    #[test]
    fn test_rsp_dma_read_multiline() {
        // Test multi-line DMA: count=2 (bit 19:12 = 1 means count-1=1 → 2 lines),
        // each line is 4 bytes, no skip between lines.
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 4096];

        // Line 0: bytes at RDRAM 0x100
        rdram[0x100] = 0x11;
        rdram[0x101] = 0x22;
        rdram[0x102] = 0x33;
        rdram[0x103] = 0x44;
        // Line 1: bytes at RDRAM 0x104 (no skip, continues immediately)
        rdram[0x104] = 0x55;
        rdram[0x105] = 0x66;
        rdram[0x106] = 0x77;
        rdram[0x107] = 0x88;

        rsp.sp_dram_addr = 0x100;
        rsp.sp_mem_addr = 0x200; // DMEM
                                 // sp_rd_len: length-1 = 3 (bits 11:0), count-1 = 1 (bits 19:12), skip = 0 (bits 31:20)
        rsp.sp_rd_len = (1 << 12) | 3;
        rsp.dma_read(&rdram);

        // Line 0 in DMEM at 0x200
        assert_eq!(rsp.read_dmem(0x200), 0x11);
        assert_eq!(rsp.read_dmem(0x201), 0x22);
        assert_eq!(rsp.read_dmem(0x202), 0x33);
        assert_eq!(rsp.read_dmem(0x203), 0x44);
        // Line 1 in DMEM at 0x204
        assert_eq!(rsp.read_dmem(0x204), 0x55);
        assert_eq!(rsp.read_dmem(0x205), 0x66);
        assert_eq!(rsp.read_dmem(0x206), 0x77);
        assert_eq!(rsp.read_dmem(0x207), 0x88);
    }

    #[test]
    fn test_rsp_dma_read_multiline_with_skip() {
        // Test multi-line DMA with non-zero DRAM skip.
        // 2 lines × 4 bytes, DRAM skip = 4 bytes between lines.
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 4096];

        // Line 0 at RDRAM 0x100
        rdram[0x100] = 0xAA;
        rdram[0x101] = 0xBB;
        rdram[0x102] = 0xCC;
        rdram[0x103] = 0xDD;
        // 4-byte gap (0x104..0x107) that should be skipped
        rdram[0x104] = 0xFF; // gap – must not appear in DMEM
        rdram[0x105] = 0xFF;
        rdram[0x106] = 0xFF;
        rdram[0x107] = 0xFF;
        // Line 1 at RDRAM 0x108 (= 0x100 + length(4) + skip(4))
        rdram[0x108] = 0x11;
        rdram[0x109] = 0x22;
        rdram[0x10A] = 0x33;
        rdram[0x10B] = 0x44;

        rsp.sp_dram_addr = 0x100;
        rsp.sp_mem_addr = 0x300; // DMEM
                                 // sp_rd_len: length-1 = 3 (bits 11:0), count-1 = 1 (bits 19:12), skip = 4 (bits 31:20)
        rsp.sp_rd_len = (4 << 20) | (1 << 12) | 3;
        rsp.dma_read(&rdram);

        // Line 0 in DMEM at 0x300
        assert_eq!(rsp.read_dmem(0x300), 0xAA);
        assert_eq!(rsp.read_dmem(0x301), 0xBB);
        assert_eq!(rsp.read_dmem(0x302), 0xCC);
        assert_eq!(rsp.read_dmem(0x303), 0xDD);
        // Line 1 in DMEM at 0x304 (skip did not copy 0xFF bytes)
        assert_eq!(rsp.read_dmem(0x304), 0x11);
        assert_eq!(rsp.read_dmem(0x305), 0x22);
        assert_eq!(rsp.read_dmem(0x306), 0x33);
        assert_eq!(rsp.read_dmem(0x307), 0x44);
    }

    #[test]
    fn test_rsp_dma_write_multiline_with_skip() {
        // Test multi-line DMA write with non-zero DRAM skip.
        // 2 lines × 4 bytes, DRAM skip = 4 bytes – RDRAM gap bytes must remain untouched.
        let mut rsp = Rsp::new();
        let mut rdram = vec![0u8; 4096];

        // Populate DMEM source data
        rsp.write_dmem(0x400, 0x11);
        rsp.write_dmem(0x401, 0x22);
        rsp.write_dmem(0x402, 0x33);
        rsp.write_dmem(0x403, 0x44);
        rsp.write_dmem(0x404, 0x55);
        rsp.write_dmem(0x405, 0x66);
        rsp.write_dmem(0x406, 0x77);
        rsp.write_dmem(0x407, 0x88);

        rsp.sp_dram_addr = 0x800;
        rsp.sp_mem_addr = 0x400; // DMEM
                                 // sp_wr_len: length-1 = 3, count-1 = 1, skip = 4
        rsp.sp_wr_len = (4 << 20) | (1 << 12) | 3;
        rsp.dma_write(&mut rdram);

        // Line 0 → RDRAM 0x800
        assert_eq!(rdram[0x800], 0x11);
        assert_eq!(rdram[0x801], 0x22);
        assert_eq!(rdram[0x802], 0x33);
        assert_eq!(rdram[0x803], 0x44);
        // Gap 0x804..0x807 must remain 0 (untouched)
        assert_eq!(rdram[0x804], 0x00);
        assert_eq!(rdram[0x807], 0x00);
        // Line 1 → RDRAM 0x808 (= 0x800 + length(4) + skip(4))
        assert_eq!(rdram[0x808], 0x55);
        assert_eq!(rdram[0x809], 0x66);
        assert_eq!(rdram[0x80A], 0x77);
        assert_eq!(rdram[0x80B], 0x88);
    }
}
