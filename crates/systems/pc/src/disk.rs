//! Disk controller for PC emulation
//!
//! Provides INT 13h disk I/O services for floppy and hard drives

/// Disk request parameters
#[allow(dead_code)]
pub struct DiskRequest {
    /// Drive number (0x00-0x7F = floppy, 0x80-0xFF = hard drive)
    pub drive: u8,
    /// Cylinder number
    pub cylinder: u16,
    /// Head number
    pub head: u8,
    /// Sector number (1-based)
    pub sector: u8,
    /// Number of sectors
    pub count: u8,
}

/// Disk controller state
pub struct DiskController {
    /// Last operation status
    status: u8,
}

impl DiskController {
    /// Create a new disk controller
    pub fn new() -> Self {
        Self { status: 0 }
    }

    /// Reset disk controller
    pub fn reset(&mut self) {
        self.status = 0;
    }

    /// Get last operation status
    #[allow(dead_code)]
    pub fn status(&self) -> u8 {
        self.status
    }

    /// Read sectors from disk
    ///
    /// Returns: Status code (0 = success)
    #[allow(dead_code)]
    pub fn read_sectors(
        &mut self,
        request: &DiskRequest,
        buffer: &mut [u8],
        disk_image: Option<&[u8]>,
    ) -> u8 {
        // If no disk image mounted, return error
        let disk_image = match disk_image {
            Some(img) => img,
            None => {
                self.status = 0x80; // Timeout (disk not ready)
                return self.status;
            }
        };

        // Calculate disk parameters based on drive type
        let (sectors_per_track, heads) = if request.drive < 0x80 {
            // Floppy: assume 1.44MB format
            (18, 2)
        } else {
            // Hard drive: assume 10MB format
            (17, 4)
        };

        // Calculate LBA (Logical Block Address)
        // SYSLINUX and some bootloaders use a hybrid addressing scheme:
        // When C=0, H=0, and S > SPT (but S < 64), treat S as a direct LBA (linear sector number)
        // This is only valid for the boot sector stage, not for normal operation
        // Otherwise use standard CHS formula: LBA = (C × HPC + H) × SPT + (S - 1)
        let lba = if request.cylinder == 0
            && request.head == 0
            && request.sector > sectors_per_track
            && request.sector < 64
        {
            // Linear sector addressing (used by SYSLINUX boot sector)
            if std::env::var("EMU_LOG_BUS").is_ok() {
                eprintln!(
                    "Disk read: Using linear addressing for S={} > SPT={}",
                    request.sector, sectors_per_track
                );
            }
            request.sector as u32 - 1
        } else if request.cylinder >= 1024
            || request.head >= heads
            || request.sector == 0
            || request.sector > 63
        {
            // Invalid CHS parameters - return error
            if std::env::var("EMU_LOG_BUS").is_ok() {
                eprintln!(
                    "Disk read: Invalid CHS - C={} H={} S={} (max C=1023, H={}, S=1-63)",
                    request.cylinder,
                    request.head,
                    request.sector,
                    heads - 1
                );
            }
            self.status = 0x01; // Invalid parameter
            return self.status;
        } else {
            // Standard CHS addressing
            ((request.cylinder as u32 * heads as u32 + request.head as u32)
                * sectors_per_track as u32)
                + (request.sector as u32 - 1)
        };

        // Each sector is 512 bytes
        let sector_size: u32 = 512;
        let offset = (lba * sector_size) as usize;

        // Log LBA calculation for debugging
        if std::env::var("EMU_LOG_BUS").is_ok() {
            eprintln!(
                "Disk read: C={} H={} S={} -> LBA={} offset=0x{:X} (SPT={}, heads={})",
                request.cylinder,
                request.head,
                request.sector,
                lba,
                offset,
                sectors_per_track,
                heads
            );
        }

        // Check if read is within bounds
        if offset + (request.count as usize * sector_size as usize) > disk_image.len() {
            self.status = 0x04; // Sector not found
            return self.status;
        }

        // Copy data from disk image to buffer
        let bytes_to_copy = (request.count as usize * sector_size as usize).min(buffer.len());
        buffer[..bytes_to_copy].copy_from_slice(&disk_image[offset..offset + bytes_to_copy]);

        // Log first few bytes of data read
        if std::env::var("EMU_LOG_BUS").is_ok() {
            eprint!("First 128 bytes read:");
            for (i, &byte) in buffer.iter().enumerate().take(128.min(bytes_to_copy)) {
                if i % 16 == 0 {
                    eprint!("\n  {:04X}:", i);
                }
                eprint!(" {:02X}", byte);
            }
            eprintln!();
        }

        self.status = 0x00; // Success
        self.status
    }

    /// Write sectors to disk
    ///
    /// Returns: Status code (0 = success)
    #[allow(dead_code)]
    pub fn write_sectors(
        &mut self,
        request: &DiskRequest,
        buffer: &[u8],
        disk_image: Option<&mut Vec<u8>>,
    ) -> u8 {
        // If no disk image mounted, return error
        let disk_image = match disk_image {
            Some(img) => img,
            None => {
                self.status = 0x80; // Timeout (disk not ready)
                return self.status;
            }
        };

        // Calculate disk parameters based on drive type
        let (sectors_per_track, heads) = if request.drive < 0x80 {
            // Floppy: assume 1.44MB format
            (18, 2)
        } else {
            // Hard drive: assume 10MB format
            (17, 4)
        };

        // Calculate LBA
        let lba = ((request.cylinder as u32 * heads as u32 + request.head as u32)
            * sectors_per_track as u32)
            + (request.sector as u32 - 1);

        let sector_size = 512;
        let offset = (lba * sector_size) as usize;

        // Check if write is within bounds
        if offset + (request.count as usize * sector_size as usize) > disk_image.len() {
            self.status = 0x04; // Sector not found
            return self.status;
        }

        // Copy data from buffer to disk image
        let bytes_to_copy = (request.count as usize * sector_size as usize).min(buffer.len());
        disk_image[offset..offset + bytes_to_copy].copy_from_slice(&buffer[..bytes_to_copy]);

        self.status = 0x00; // Success
        self.status
    }

    /// Read sectors using LBA (Logical Block Addressing)
    ///
    /// Returns: Status code (0 = success)
    pub fn read_sectors_lba(
        &mut self,
        lba: u32,
        count: u8,
        buffer: &mut [u8],
        disk_image: Option<&[u8]>,
    ) -> u8 {
        // If no disk image mounted, return error
        let disk_image = match disk_image {
            Some(img) => img,
            None => {
                self.status = 0x80; // Timeout (disk not ready)
                return self.status;
            }
        };

        // Each sector is 512 bytes
        let sector_size = 512;
        let offset = (lba * sector_size) as usize;

        // Check if read is within bounds
        if offset + (count as usize * sector_size as usize) > disk_image.len() {
            self.status = 0x04; // Sector not found
            return self.status;
        }

        // Copy data from disk image to buffer
        let bytes_to_copy = (count as usize * sector_size as usize).min(buffer.len());
        buffer[..bytes_to_copy].copy_from_slice(&disk_image[offset..offset + bytes_to_copy]);

        self.status = 0x00; // Success
        self.status
    }

    /// Write sectors using LBA (Logical Block Addressing)
    ///
    /// Returns: Status code (0 = success)
    pub fn write_sectors_lba(
        &mut self,
        lba: u32,
        count: u8,
        buffer: &[u8],
        disk_image: Option<&mut Vec<u8>>,
    ) -> u8 {
        // If no disk image mounted, return error
        let disk_image = match disk_image {
            Some(img) => img,
            None => {
                self.status = 0x80; // Timeout (disk not ready)
                return self.status;
            }
        };

        // Each sector is 512 bytes
        let sector_size = 512;
        let offset = (lba * sector_size) as usize;

        // Check if write is within bounds
        if offset + (count as usize * sector_size as usize) > disk_image.len() {
            self.status = 0x04; // Sector not found
            return self.status;
        }

        // Copy data from buffer to disk image
        let bytes_to_copy = (count as usize * sector_size as usize).min(buffer.len());
        disk_image[offset..offset + bytes_to_copy].copy_from_slice(&buffer[..bytes_to_copy]);

        self.status = 0x00; // Success
        self.status
    }

    /// Get drive parameters
    ///
    /// Returns: (cylinders, sectors_per_track, heads) or None if invalid drive
    #[allow(dead_code)]
    pub fn get_drive_params(drive: u8) -> Option<(u16, u8, u8)> {
        if drive < 0x80 {
            // Floppy drive - 1.44MB format
            Some((80, 18, 2))
        } else if drive == 0x80 {
            // Hard drive C: - 10MB
            Some((306, 17, 4))
        } else {
            None
        }
    }
}

impl Default for DiskController {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard floppy disk formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloppyFormat {
    /// 360KB - 5.25" DD (40 tracks, 9 sectors, 2 heads)
    Floppy360K,
    /// 720KB - 3.5" DD (80 tracks, 9 sectors, 2 heads)
    Floppy720K,
    /// 1.2MB - 5.25" HD (80 tracks, 15 sectors, 2 heads)
    Floppy1_2M,
    /// 1.44MB - 3.5" HD (80 tracks, 18 sectors, 2 heads)
    Floppy1_44M,
}

impl FloppyFormat {
    /// Get the size in bytes for this format
    pub fn size_bytes(&self) -> usize {
        match self {
            FloppyFormat::Floppy360K => 368640,   // 360 * 1024
            FloppyFormat::Floppy720K => 737280,   // 720 * 1024
            FloppyFormat::Floppy1_2M => 1228800,  // 1200 * 1024
            FloppyFormat::Floppy1_44M => 1474560, // 1440 * 1024
        }
    }

    /// Get the geometry (cylinders, sectors_per_track, heads) for this format
    pub fn geometry(&self) -> (u16, u8, u8) {
        match self {
            FloppyFormat::Floppy360K => (40, 9, 2),
            FloppyFormat::Floppy720K => (80, 9, 2),
            FloppyFormat::Floppy1_2M => (80, 15, 2),
            FloppyFormat::Floppy1_44M => (80, 18, 2),
        }
    }
}

/// Standard hard drive formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardDriveFormat {
    /// 10MB hard drive (306 cylinders, 17 sectors, 4 heads)
    HardDrive10M,
    /// 20MB hard drive (612 cylinders, 17 sectors, 4 heads)
    HardDrive20M,
    /// 40MB hard drive (980 cylinders, 17 sectors, 5 heads)
    HardDrive40M,
}

impl HardDriveFormat {
    /// Get the size in bytes for this format
    pub fn size_bytes(&self) -> usize {
        match self {
            HardDriveFormat::HardDrive10M => 10653696, // ~10MB
            HardDriveFormat::HardDrive20M => 21307392, // ~20MB
            HardDriveFormat::HardDrive40M => 42618880, // ~40MB
        }
    }

    /// Get the geometry (cylinders, sectors_per_track, heads) for this format
    pub fn geometry(&self) -> (u16, u8, u8) {
        match self {
            HardDriveFormat::HardDrive10M => (306, 17, 4),
            HardDriveFormat::HardDrive20M => (612, 17, 4),
            HardDriveFormat::HardDrive40M => (980, 17, 5),
        }
    }
}

/// Create a blank floppy disk image
pub fn create_blank_floppy(format: FloppyFormat) -> Vec<u8> {
    vec![0; format.size_bytes()]
}

/// Create a blank hard drive image
pub fn create_blank_hard_drive(format: HardDriveFormat) -> Vec<u8> {
    vec![0; format.size_bytes()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_creation() {
        let controller = DiskController::new();
        assert_eq!(controller.status(), 0);
    }

    #[test]
    fn test_read_no_disk() {
        let mut controller = DiskController::new();
        let mut buffer = vec![0; 512];

        let request = DiskRequest {
            drive: 0x00,
            cylinder: 0,
            head: 0,
            sector: 1,
            count: 1,
        };

        let status = controller.read_sectors(&request, &mut buffer, None);
        assert_eq!(status, 0x80); // Timeout - no disk
    }

    #[test]
    fn test_read_floppy_sector() {
        let mut controller = DiskController::new();

        // Create a minimal floppy image (1.44MB = 1,474,560 bytes)
        let mut disk_image = vec![0; 1_474_560];

        // Fill first sector with pattern
        for (i, byte) in disk_image.iter_mut().enumerate().take(512) {
            *byte = (i % 256) as u8;
        }

        let mut buffer = vec![0; 512];

        let request = DiskRequest {
            drive: 0x00,
            cylinder: 0,
            head: 0,
            sector: 1,
            count: 1,
        };

        let status = controller.read_sectors(&request, &mut buffer, Some(&disk_image));

        assert_eq!(status, 0x00); // Success
        assert_eq!(buffer[0], 0);
        assert_eq!(buffer[255], 255);
        assert_eq!(buffer[256], 0);
    }

    #[test]
    fn test_write_floppy_sector() {
        let mut controller = DiskController::new();

        // Create a minimal floppy image
        let mut disk_image = vec![0; 1_474_560];

        // Create pattern to write
        let buffer: Vec<u8> = (0..512).map(|i| (i % 256) as u8).collect();

        let request = DiskRequest {
            drive: 0x00,
            cylinder: 0,
            head: 0,
            sector: 1,
            count: 1,
        };

        let status = controller.write_sectors(&request, &buffer, Some(&mut disk_image));

        assert_eq!(status, 0x00); // Success
        assert_eq!(disk_image[0], 0);
        assert_eq!(disk_image[255], 255);
        assert_eq!(disk_image[256], 0);
    }

    #[test]
    fn test_read_out_of_bounds() {
        let mut controller = DiskController::new();

        // Small disk image
        let disk_image = vec![0; 1024];
        let mut buffer = vec![0; 512];

        // Try to read beyond disk size
        let request = DiskRequest {
            drive: 0x00,
            cylinder: 10,
            head: 0,
            sector: 1,
            count: 1,
        };

        let status = controller.read_sectors(&request, &mut buffer, Some(&disk_image));

        assert_eq!(status, 0x04); // Sector not found
    }

    #[test]
    fn test_get_floppy_params() {
        let params = DiskController::get_drive_params(0x00);
        assert!(params.is_some());

        let (cylinders, sectors, heads) = params.unwrap();
        assert_eq!(cylinders, 80);
        assert_eq!(sectors, 18);
        assert_eq!(heads, 2);
    }

    #[test]
    fn test_get_hard_drive_params() {
        let params = DiskController::get_drive_params(0x80);
        assert!(params.is_some());

        let (cylinders, sectors, heads) = params.unwrap();
        assert_eq!(cylinders, 306);
        assert_eq!(sectors, 17);
        assert_eq!(heads, 4);
    }

    #[test]
    fn test_reset() {
        let mut controller = DiskController::new();
        controller.status = 0xFF;

        controller.reset();
        assert_eq!(controller.status(), 0);
    }

    #[test]
    fn test_create_blank_floppy_360k() {
        let disk = create_blank_floppy(FloppyFormat::Floppy360K);
        assert_eq!(disk.len(), 368640);
        assert_eq!(disk[0], 0);
        assert_eq!(disk[disk.len() - 1], 0);
    }

    #[test]
    fn test_create_blank_floppy_720k() {
        let disk = create_blank_floppy(FloppyFormat::Floppy720K);
        assert_eq!(disk.len(), 737280);
    }

    #[test]
    fn test_create_blank_floppy_1_44m() {
        let disk = create_blank_floppy(FloppyFormat::Floppy1_44M);
        assert_eq!(disk.len(), 1474560);
    }

    #[test]
    fn test_create_blank_hard_drive_10m() {
        let disk = create_blank_hard_drive(HardDriveFormat::HardDrive10M);
        assert_eq!(disk.len(), 10653696);
        assert_eq!(disk[0], 0);
    }

    #[test]
    fn test_create_blank_hard_drive_20m() {
        let disk = create_blank_hard_drive(HardDriveFormat::HardDrive20M);
        assert_eq!(disk.len(), 21307392);
    }

    #[test]
    fn test_floppy_format_geometry() {
        let (c, s, h) = FloppyFormat::Floppy1_44M.geometry();
        assert_eq!(c, 80);
        assert_eq!(s, 18);
        assert_eq!(h, 2);
    }

    #[test]
    fn test_hard_drive_format_geometry() {
        let (c, s, h) = HardDriveFormat::HardDrive10M.geometry();
        assert_eq!(c, 306);
        assert_eq!(s, 17);
        assert_eq!(h, 4);
    }
}
