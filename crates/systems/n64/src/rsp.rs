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
#[allow(dead_code)]
const SP_STATUS_SIG0: u32 = 0x080; // Signal 0
#[allow(dead_code)]
const SP_STATUS_SIG1: u32 = 0x100; // Signal 1
#[allow(dead_code)]
const SP_STATUS_SIG2: u32 = 0x200; // Signal 2
#[allow(dead_code)]
const SP_STATUS_SIG3: u32 = 0x400; // Signal 3
#[allow(dead_code)]
const SP_STATUS_SIG4: u32 = 0x800; // Signal 4
#[allow(dead_code)]
const SP_STATUS_SIG5: u32 = 0x1000; // Signal 5
#[allow(dead_code)]
const SP_STATUS_SIG6: u32 = 0x2000; // Signal 6
#[allow(dead_code)]
const SP_STATUS_SIG7: u32 = 0x4000; // Signal 7

use super::rdp::Rdp;
use super::rsp_hle::RspHle;

/// RSP (Reality Signal Processor) state
pub struct Rsp {
    /// 4KB DMEM (Data Memory)
    dmem: [u8; 4096],

    /// 4KB IMEM (Instruction Memory)
    imem: [u8; 4096],

    /// Program counter (not yet used - for future microcode execution)
    #[allow(dead_code)]
    pc: u32,

    /// RSP registers
    sp_mem_addr: u32,
    sp_dram_addr: u32,
    sp_rd_len: u32,
    sp_wr_len: u32,
    sp_status: u32,
    sp_dma_full: u32,
    sp_dma_busy: u32,
    sp_semaphore: u32,

    /// High-level emulation state
    hle: RspHle,
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
            sp_semaphore: 0,
            hle: RspHle::new(),
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
        self.sp_semaphore = 0;
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
                // Reading semaphore clears it (returns 1 if was set, 0 if was clear)
                // For now, stub implementation always returns 0
                0
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
                self.sp_rd_len = value & 0x0FFF;
                self.dma_read(rdram);
            }
            SP_WR_LEN => {
                // DMA write from RSP memory to RDRAM
                self.sp_wr_len = value & 0x0FFF;
                self.dma_write(rdram);
            }
            SP_STATUS => {
                // Status register write (control bits)
                // Bit 0: Clear halt
                if value & 0x0001 != 0 {
                    self.sp_status &= !SP_STATUS_HALT;
                }
                // Bit 1: Set halt
                if value & 0x0002 != 0 {
                    self.sp_status |= SP_STATUS_HALT;
                }
                // Bit 2: Clear broke
                if value & 0x0004 != 0 {
                    self.sp_status &= !SP_STATUS_BROKE;
                }
                // Other bits control interrupts, signals, etc. (not implemented)
            }
            SP_SEMAPHORE => {
                // Writing any value to semaphore sets it
                self.sp_semaphore = 1;
            }
            _ => {}
        }
    }

    /// DMA read from RDRAM to RSP memory
    fn dma_read(&mut self, rdram: &[u8]) {
        let length = (self.sp_rd_len & 0xFFF) + 1;
        let dram_addr = (self.sp_dram_addr & 0x00FFFFFF) as usize;
        let mem_addr = (self.sp_mem_addr & 0x1FFF) as usize;
        let is_imem = (self.sp_mem_addr & 0x1000) != 0;

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

        // If we just loaded IMEM, detect microcode
        if is_imem {
            self.hle.detect_microcode(&self.imem);
        }
    }

    /// DMA write from RSP memory to RDRAM
    fn dma_write(&mut self, rdram: &mut [u8]) {
        let length = (self.sp_wr_len & 0xFFF) + 1;
        let dram_addr = (self.sp_dram_addr & 0x00FFFFFF) as usize;
        let mem_addr = (self.sp_mem_addr & 0x1FFF) as usize;
        let is_imem = (self.sp_mem_addr & 0x1000) != 0;

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
    }

    /// Execute RSP task via HLE
    /// Called when RSP is un-halted by writing to SP_STATUS
    pub fn execute_task(&mut self, rdram: &[u8], rdp: &mut Rdp) -> u32 {
        // Check if RSP is halted
        if self.sp_status & SP_STATUS_HALT != 0 {
            return 0;
        }

        // Execute HLE task
        let cycles = self.hle.execute_task(&self.dmem, rdram, rdp);

        // Set broke flag and halt after task completion
        self.sp_status |= SP_STATUS_BROKE | SP_STATUS_HALT;

        cycles
    }

    /// Get current microcode type (for debugging/monitoring)
    #[allow(dead_code)]
    pub fn microcode(&self) -> super::rsp_hle::MicrocodeType {
        self.hle.microcode()
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
}
