//! PC CPU wrapper
//!
//! This module wraps the core 8086 CPU with PC-specific initialization and state.

use crate::bus::PcBus;
use emu_core::cpu_8086::{Cpu8086, Memory8086};

/// PC CPU wrapper
pub struct PcCpu {
    cpu: Cpu8086<PcBus>,
}

impl PcCpu {
    /// Create a new PC CPU with the given bus
    pub fn new(bus: PcBus) -> Self {
        let mut cpu = Cpu8086::new(bus);

        // IBM PC/XT boots at CS:IP = 0xFFFF:0x0000 (physical address 0xFFFF0)
        // This is the BIOS entry point
        cpu.cs = 0xFFFF;
        cpu.ip = 0x0000;

        // Initialize stack pointer
        cpu.ss = 0x0000;
        cpu.sp = 0xFFFE;

        // Initialize data segments
        cpu.ds = 0x0000;
        cpu.es = 0x0000;

        Self { cpu }
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        self.cpu.reset();

        // Restore PC boot state
        self.cpu.cs = 0xFFFF;
        self.cpu.ip = 0x0000;
        self.cpu.ss = 0x0000;
        self.cpu.sp = 0xFFFE;
        self.cpu.ds = 0x0000;
        self.cpu.es = 0x0000;
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        // Check if the next instruction is INT 13h (BIOS disk services)
        // Opcode 0xCD (INT) followed by 0x13
        let cs = self.cpu.cs;
        let ip = self.cpu.ip;
        let physical_addr = ((cs as u32) << 4) + (ip as u32);

        // Peek at the instruction without advancing IP
        let opcode = self.cpu.memory.read(physical_addr);
        if opcode == 0xCD {
            // This is an INT instruction, check the interrupt number
            let int_num = self.cpu.memory.read(physical_addr + 1);
            if int_num == 0x13 {
                // Handle INT 13h - BIOS disk services
                return self.handle_int13h();
            }
        }

        // Otherwise, execute normally
        self.cpu.step()
    }

    /// Handle INT 13h BIOS disk services
    fn handle_int13h(&mut self) -> u32 {
        // Skip the INT 13h instruction (2 bytes: 0xCD 0x13)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        match ah {
            0x00 => self.int13h_reset(),
            0x02 => self.int13h_read_sectors(),
            0x03 => self.int13h_write_sectors(),
            0x08 => self.int13h_get_drive_params(),
            _ => {
                // Unsupported function - set error in AH
                self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid function
                self.set_carry_flag(true);
                51 // Approximate INT instruction timing
            }
        }
    }

    /// INT 13h, AH=00h: Reset disk system
    fn int13h_reset(&mut self) -> u32 {
        // Get drive number from DL
        let _dl = (self.cpu.dx & 0xFF) as u8;

        // Reset the disk controller
        self.cpu.memory.disk_controller_mut().reset();

        // Clear AH (status = success)
        self.cpu.ax &= 0x00FF;

        // Clear carry flag (success)
        self.set_carry_flag(false);

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=02h: Read sectors
    fn int13h_read_sectors(&mut self) -> u32 {
        use crate::disk::DiskRequest;

        // AL = number of sectors to read
        let count = (self.cpu.ax & 0xFF) as u8;

        // CH = cylinder (low 8 bits)
        // CL = sector number (bits 0-5), high 2 bits of cylinder (bits 6-7)
        let ch = ((self.cpu.cx >> 8) & 0xFF) as u8;
        let cl = (self.cpu.cx & 0xFF) as u8;
        let cylinder = ((cl as u16 & 0xC0) << 2) | (ch as u16);
        let sector = cl & 0x3F;

        // DH = head number
        let head = ((self.cpu.dx >> 8) & 0xFF) as u8;

        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        // ES:BX = buffer address
        let buffer_seg = self.cpu.es;
        let buffer_offset = self.cpu.bx;

        // Create disk request
        let request = DiskRequest {
            drive,
            cylinder,
            head,
            sector,
            count,
        };

        // Prepare buffer
        let buffer_size = (count as usize) * 512;
        let mut buffer = vec![0u8; buffer_size];

        // Perform read using bus helper method
        let status = self.cpu.memory.disk_read(&request, &mut buffer);

        // Copy buffer to memory at ES:BX
        if status == 0x00 {
            for (i, &byte) in buffer.iter().enumerate() {
                let offset = buffer_offset.wrapping_add(i as u16);
                self.cpu.write_byte(buffer_seg, offset, byte);
            }
        }

        // Set AH = status
        self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u16) << 8);

        // Set carry flag based on status
        self.set_carry_flag(status != 0x00);

        // AL = number of sectors read (on success)
        if status == 0x00 {
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (count as u16);
        }

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=03h: Write sectors
    fn int13h_write_sectors(&mut self) -> u32 {
        use crate::disk::DiskRequest;

        // AL = number of sectors to write
        let count = (self.cpu.ax & 0xFF) as u8;

        // CH = cylinder (low 8 bits)
        // CL = sector number (bits 0-5), high 2 bits of cylinder (bits 6-7)
        let ch = ((self.cpu.cx >> 8) & 0xFF) as u8;
        let cl = (self.cpu.cx & 0xFF) as u8;
        let cylinder = ((cl as u16 & 0xC0) << 2) | (ch as u16);
        let sector = cl & 0x3F;

        // DH = head number
        let head = ((self.cpu.dx >> 8) & 0xFF) as u8;

        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        // ES:BX = buffer address
        let buffer_seg = self.cpu.es;
        let buffer_offset = self.cpu.bx;

        // Read data from memory at ES:BX
        let buffer_size = (count as usize) * 512;
        let mut buffer = vec![0u8; buffer_size];
        for (i, byte) in buffer.iter_mut().enumerate() {
            let offset = buffer_offset.wrapping_add(i as u16);
            *byte = self.cpu.read_byte(buffer_seg, offset);
        }

        // Create disk request
        let request = DiskRequest {
            drive,
            cylinder,
            head,
            sector,
            count,
        };

        // Perform write using bus helper method
        let status = self.cpu.memory.disk_write(&request, &buffer);

        // Set AH = status
        self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u16) << 8);

        // Set carry flag based on status
        self.set_carry_flag(status != 0x00);

        // AL = number of sectors written (on success)
        if status == 0x00 {
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (count as u16);
        }

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=08h: Get drive parameters
    fn int13h_get_drive_params(&mut self) -> u32 {
        use crate::disk::DiskController;

        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        // Get drive parameters
        if let Some((cylinders, sectors_per_track, heads)) = DiskController::get_drive_params(drive)
        {
            // BL = drive type (for floppies)
            if drive < 0x80 {
                self.cpu.bx = (self.cpu.bx & 0xFF00) | 0x04; // 1.44MB floppy
            } else {
                self.cpu.bx &= 0xFF00; // Hard drive
            }

            // CH = low 8 bits of maximum cylinder number
            // CL = sectors per track (bits 0-5), high 2 bits of cylinders (bits 6-7)
            let max_cylinder = cylinders - 1; // 0-based
            let ch = (max_cylinder & 0xFF) as u8;
            let cl_high = ((max_cylinder >> 2) & 0xC0) as u8;
            let cl = cl_high | sectors_per_track;

            self.cpu.cx = ((ch as u16) << 8) | (cl as u16);

            // DH = maximum head number (0-based)
            // DL = number of drives
            self.cpu.dx = (((heads - 1) as u16) << 8) | 0x01;

            // ES:DI = pointer to disk parameter table (set to 0x0000:0x0000 for now)
            self.cpu.es = 0x0000;
            self.cpu.di = 0x0000;

            // AH = 0 (success)
            self.cpu.ax &= 0x00FF;

            // Clear carry flag (success)
            self.set_carry_flag(false);
        } else {
            // Invalid drive
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid function
            self.set_carry_flag(true);
        }

        51 // Approximate INT instruction timing
    }

    /// Set or clear the carry flag
    fn set_carry_flag(&mut self, value: bool) {
        const FLAG_CF: u16 = 0x0001;
        if value {
            self.cpu.flags |= FLAG_CF;
        } else {
            self.cpu.flags &= !FLAG_CF;
        }
    }

    /// Get a reference to the bus
    pub fn bus(&self) -> &PcBus {
        &self.cpu.memory
    }

    /// Get a mutable reference to the bus
    pub fn bus_mut(&mut self) -> &mut PcBus {
        &mut self.cpu.memory
    }

    /// Get CPU register state for debugging/save states
    pub fn get_registers(&self) -> CpuRegisters {
        CpuRegisters {
            ax: self.cpu.ax,
            bx: self.cpu.bx,
            cx: self.cpu.cx,
            dx: self.cpu.dx,
            si: self.cpu.si,
            di: self.cpu.di,
            bp: self.cpu.bp,
            sp: self.cpu.sp,
            cs: self.cpu.cs,
            ds: self.cpu.ds,
            es: self.cpu.es,
            ss: self.cpu.ss,
            ip: self.cpu.ip,
            flags: self.cpu.flags,
        }
    }

    /// Set CPU register state (for loading save states)
    pub fn set_registers(&mut self, regs: &CpuRegisters) {
        self.cpu.ax = regs.ax;
        self.cpu.bx = regs.bx;
        self.cpu.cx = regs.cx;
        self.cpu.dx = regs.dx;
        self.cpu.si = regs.si;
        self.cpu.di = regs.di;
        self.cpu.bp = regs.bp;
        self.cpu.sp = regs.sp;
        self.cpu.cs = regs.cs;
        self.cpu.ds = regs.ds;
        self.cpu.es = regs.es;
        self.cpu.ss = regs.ss;
        self.cpu.ip = regs.ip;
        self.cpu.flags = regs.flags;
    }

    /// Get total cycles executed
    #[allow(dead_code)]
    pub fn cycles(&self) -> u64 {
        self.cpu.cycles
    }
}

/// CPU register state for save/load
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CpuRegisters {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub bp: u16,
    pub sp: u16,
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub ip: u16,
    pub flags: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_initialization() {
        let bus = PcBus::new();
        let cpu = PcCpu::new(bus);

        // Check PC boot state
        assert_eq!(cpu.cpu.cs, 0xFFFF);
        assert_eq!(cpu.cpu.ip, 0x0000);
        assert_eq!(cpu.cpu.ss, 0x0000);
        assert_eq!(cpu.cpu.sp, 0xFFFE);
    }

    #[test]
    fn test_cpu_reset() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Modify some registers
        cpu.cpu.ax = 0x1234;
        cpu.cpu.cs = 0x0100;

        cpu.reset();

        // Should be back to boot state
        assert_eq!(cpu.cpu.ax, 0x0000);
        assert_eq!(cpu.cpu.cs, 0xFFFF);
        assert_eq!(cpu.cpu.ip, 0x0000);
    }

    #[test]
    fn test_register_save_load() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        cpu.cpu.ax = 0x1234;
        cpu.cpu.bx = 0x5678;
        cpu.cpu.cs = 0xABCD;

        let regs = cpu.get_registers();
        assert_eq!(regs.ax, 0x1234);
        assert_eq!(regs.bx, 0x5678);
        assert_eq!(regs.cs, 0xABCD);

        cpu.reset();
        assert_eq!(cpu.cpu.ax, 0x0000);

        cpu.set_registers(&regs);
        assert_eq!(cpu.cpu.ax, 0x1234);
        assert_eq!(cpu.cpu.bx, 0x5678);
        assert_eq!(cpu.cpu.cs, 0xABCD);
    }

    #[test]
    fn test_int13h_reset() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to a RAM location where we can write test code
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction at current IP
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=00h (reset)
        cpu.cpu.ax = 0x0000; // AH=00h (reset)
        cpu.cpu.dx = 0x0080; // DL=80h (hard drive)

        // Execute INT 13h
        let cycles = cpu.step();

        // Should have executed and advanced IP by 2
        assert_eq!(cpu.cpu.ip, ip.wrapping_add(2));

        // AH should be 0 (success)
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00);

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Should have taken cycles
        assert!(cycles > 0);
    }

    #[test]
    fn test_int13h_read_sectors_no_disk() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read sectors)
        cpu.cpu.ax = 0x0201; // AH=02h (read), AL=01 (1 sector)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should fail with timeout (no disk mounted)
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x80); // Status = timeout

        // Carry flag should be set (error)
        assert_eq!(cpu.cpu.flags & 0x0001, 1);
    }

    #[test]
    fn test_int13h_read_sectors_success() {
        let mut bus = PcBus::new();

        // Create a floppy image with test data
        let mut floppy = vec![0; 1474560]; // 1.44MB

        // Fill first sector with test pattern
        for (i, byte) in floppy.iter_mut().enumerate().take(512) {
            *byte = (i % 256) as u8;
        }

        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read sectors)
        cpu.cpu.ax = 0x0201; // AH=02h (read), AL=01 (1 sector)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x01); // AL = sectors read

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Verify data was copied to buffer
        let buffer_addr = 0x7C00;
        assert_eq!(cpu.cpu.memory.read(buffer_addr), 0);
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 255), 255);
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 256), 0);
    }

    #[test]
    fn test_int13h_write_sectors() {
        let mut bus = PcBus::new();

        // Create a blank floppy image
        let floppy = vec![0; 1474560]; // 1.44MB
        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write test data to memory at 0x0000:0x7C00
        let buffer_addr = 0x7C00;
        for i in 0..512 {
            cpu.cpu.memory.write(buffer_addr + i, (i % 256) as u8);
        }

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=03h (write sectors)
        cpu.cpu.ax = 0x0301; // AH=03h (write), AL=01 (1 sector)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x01); // AL = sectors written

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Verify data was written to floppy
        let floppy = cpu.cpu.memory.floppy_a().unwrap();
        assert_eq!(floppy[0], 0);
        assert_eq!(floppy[255], 255);
        assert_eq!(floppy[256], 0);
    }

    #[test]
    fn test_int13h_get_drive_params_floppy() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=08h (get drive params)
        cpu.cpu.ax = 0x0800; // AH=08h (get drive params)
        cpu.cpu.dx = 0x0000; // DL=00 (floppy A)

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Check returned parameters (1.44MB floppy: 80 cylinders, 18 sectors, 2 heads)
        let ch = (cpu.cpu.cx >> 8) & 0xFF;
        let cl = cpu.cpu.cx & 0xFF;
        let sectors = cl & 0x3F;
        let cylinder_high = (cl & 0xC0) >> 6;
        let cylinder = (cylinder_high << 8) | ch;

        assert_eq!(cylinder, 79); // Max cylinder (0-based)
        assert_eq!(sectors, 18); // Sectors per track

        let dh = (cpu.cpu.dx >> 8) & 0xFF;
        assert_eq!(dh, 1); // Max head (0-based, so 2 heads = 0-1)

        // BL should indicate floppy type
        let bl = cpu.cpu.bx & 0xFF;
        assert_eq!(bl, 0x04); // 1.44MB floppy
    }

    #[test]
    fn test_int13h_get_drive_params_hard_drive() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=08h (get drive params)
        cpu.cpu.ax = 0x0800; // AH=08h (get drive params)
        cpu.cpu.dx = 0x0080; // DL=80h (hard drive C)

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Check returned parameters (10MB drive: 306 cylinders, 17 sectors, 4 heads)
        let ch = (cpu.cpu.cx >> 8) & 0xFF;
        let cl = cpu.cpu.cx & 0xFF;
        let sectors = cl & 0x3F;
        let cylinder_high = (cl & 0xC0) >> 6;
        let cylinder = (cylinder_high << 8) | ch;

        assert_eq!(cylinder, 305); // Max cylinder (0-based)
        assert_eq!(sectors, 17); // Sectors per track

        let dh = (cpu.cpu.dx >> 8) & 0xFF;
        assert_eq!(dh, 3); // Max head (0-based, so 4 heads = 0-3)
    }

    #[test]
    fn test_int13h_unsupported_function() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for unsupported function (AH=FFh)
        cpu.cpu.ax = 0xFF00; // AH=FFh (unsupported)

        // Execute INT 13h
        cpu.step();

        // Should fail with invalid function
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x01); // Status = invalid function

        // Carry flag should be set (error)
        assert_eq!(cpu.cpu.flags & 0x0001, 1);
    }

    #[test]
    fn test_int13h_read_multiple_sectors() {
        let mut bus = PcBus::new();

        // Create a floppy image with test data
        let mut floppy = vec![0; 1474560]; // 1.44MB

        // Fill first 3 sectors with different patterns
        for sector in 0..3 {
            for i in 0..512 {
                floppy[sector * 512 + i] = ((sector * 100 + i) % 256) as u8;
            }
        }

        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read 3 sectors)
        cpu.cpu.ax = 0x0203; // AH=02h (read), AL=03 (3 sectors)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x03); // AL = 3 sectors read

        // Verify all 3 sectors were read
        let buffer_addr = 0x7C00;
        assert_eq!(cpu.cpu.memory.read(buffer_addr), 0); // Sector 0, byte 0
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 512), 100); // Sector 1, byte 0
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 1024), 200); // Sector 2, byte 0
    }
}
