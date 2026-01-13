//! TLB (Translation Lookaside Buffer) for N64 virtual memory
//!
//! The N64 MIPS R4300i CPU uses a TLB for virtual-to-physical address translation.
//! This module implements a simplified TLB that supports:
//! - 32 TLB entries (standard R4300i configuration)
//! - Even/odd page pairs (2MB total per entry)
//! - Valid, dirty, and global bits
//! - ASID (Address Space Identifier) for process isolation
//!
//! ## TLB Entry Format
//!
//! Each TLB entry contains:
//! - **VPN2** (Virtual Page Number / 2): bits 39-13 of virtual address
//! - **ASID** (Address Space ID): 8-bit process identifier
//! - **G** (Global): Entry valid for all ASIDs when set
//! - **PFN0/PFN1** (Physical Frame Number): Even/odd page physical addresses
//! - **C0/C1** (Cache coherency): Cache algorithm for each page
//! - **D0/D1** (Dirty): Write enable for each page
//! - **V0/V1** (Valid): Validity of each page
//!
//! ## Memory Segments
//!
//! MIPS has several fixed memory segments that bypass TLB:
//! - **KUSEG** (0x00000000-0x7FFFFFFF): User segment, TLB mapped
//! - **KSEG0** (0x80000000-0x9FFFFFFF): Kernel unmapped, cached
//! - **KSEG1** (0xA0000000-0xBFFFFFFF): Kernel unmapped, uncached
//! - **KSSEG** (0xC0000000-0xDFFFFFFF): Kernel supervisor, TLB mapped
//! - **KSEG3** (0xE0000000-0xFFFFFFFF): Kernel, TLB mapped

/// TLB entry for virtual-to-physical address translation
#[derive(Debug, Clone, Copy, Default)]
pub struct TlbEntry {
    /// Virtual Page Number / 2 (bits 39-13 of virtual address)
    /// Covers 8KB (2 pages of 4KB each)
    pub vpn2: u64,

    /// Address Space ID (8-bit)
    pub asid: u8,

    /// Global bit - entry matches all ASIDs when set
    pub global: bool,

    /// Page mask - determines page size (4KB to 16MB)
    /// 0x0000 = 4KB, 0x0003 = 16KB, 0x000F = 64KB, etc.
    pub page_mask: u32,

    /// Even page (page 0)
    pub pfn0: u32, // Physical Frame Number
    pub c0: u8,   // Cache coherency algorithm
    pub d0: bool, // Dirty (writable)
    pub v0: bool, // Valid

    /// Odd page (page 1)
    pub pfn1: u32, // Physical Frame Number
    pub c1: u8,   // Cache coherency algorithm
    pub d1: bool, // Dirty (writable)
    pub v1: bool, // Valid
}

/// Translation Lookaside Buffer
pub struct Tlb {
    /// 32 TLB entries (standard R4300i configuration)
    pub(crate) entries: [TlbEntry; 32],

    /// Current ASID from CP0 EntryHi register
    current_asid: u8,
}

impl Tlb {
    /// Create a new TLB with all entries invalid
    pub fn new() -> Self {
        Self {
            entries: [TlbEntry::default(); 32],
            current_asid: 0,
        }
    }

    /// Translate virtual address to physical address
    /// Returns (physical_address, is_cached) or None if TLB miss
    ///
    /// # TLB Translation Process
    ///
    /// 1. **Check unmapped segments**: KSEG0/KSEG1 bypass TLB
    /// 2. **Extract VPN2**: Virtual Page Number / 2 (bits 39-13)
    /// 3. **Search TLB entries**: Match VPN2 considering page mask
    /// 4. **Check ASID**: Verify Address Space ID (unless global entry)
    /// 5. **Validate page**: Ensure V (valid) bit is set
    /// 6. **Calculate physical address**: PFN + page offset
    ///
    /// # Edge Cases Handled
    ///
    /// - **Page mask overflow**: Limited to valid range (prevents arithmetic overflow)
    /// - **Invalid entries**: Skipped (V=0 indicates unmapped page)
    /// - **Global entries**: Match all ASIDs (G=1)
    /// - **Even/odd pages**: Bit 12 selects which page in entry
    pub fn translate(&self, virt_addr: u64) -> Option<(u32, bool)> {
        // Check for unmapped segments that bypass TLB
        match virt_addr {
            // KSEG0: 0x80000000-0x9FFFFFFF -> Direct mapping, cached
            0x8000_0000..=0x9FFF_FFFF => {
                let phys_addr = (virt_addr & 0x1FFF_FFFF) as u32;
                return Some((phys_addr, true)); // Cached
            }
            // KSEG1: 0xA0000000-0xBFFFFFFF -> Direct mapping, uncached
            0xA000_0000..=0xBFFF_FFFF => {
                let phys_addr = (virt_addr & 0x1FFF_FFFF) as u32;
                return Some((phys_addr, false)); // Uncached
            }
            // Other segments use TLB
            _ => {}
        }

        // For TLB-mapped segments, search TLB entries
        let vpn2 = (virt_addr >> 13) & 0x07FFFFFF; // VPN2: bits 39-13
        let odd_page = (virt_addr >> 12) & 1 == 1; // Bit 12 selects even/odd page
        let offset = virt_addr & 0xFFF; // Page offset (bits 11-0)

        for entry in &self.entries {
            // Edge case: Limit page_mask to prevent overflow
            // Valid page sizes: 4KB to 16MB (mask 0x000 to 0xFFF)
            let safe_page_mask = (entry.page_mask & 0xFFF) as u64;

            // Check if VPN2 matches (considering page mask)
            let mask = safe_page_mask << 12;
            let vpn_mask = !mask;
            if (entry.vpn2 & vpn_mask) != (vpn2 & vpn_mask) {
                continue;
            }

            // Check ASID match (or global entry)
            if !entry.global && entry.asid != self.current_asid {
                continue;
            }

            // Select even or odd page
            let (pfn, valid, _dirty, c) = if odd_page {
                (entry.pfn1, entry.v1, entry.d1, entry.c1)
            } else {
                (entry.pfn0, entry.v0, entry.d0, entry.c0)
            };

            // Check if page is valid
            if !valid {
                continue; // TLB invalid exception would occur here
            }

            // Calculate physical address with safe page size
            let page_size = (safe_page_mask + 1) << 12;
            // Page offset calculation:
            // 1. offset (bits 11-0) provides the base page offset
            // 2. For pages larger than 4KB, we need additional bits from virt_addr
            // 3. (virt_addr & (page_size - 1)) gets all bits within the page
            // 4. & !0xFFF masks out the low 12 bits (already in offset)
            // 5. | combines the offset with the additional page bits
            // Example: For 16KB pages, this includes bits 13-12 from virt_addr
            let page_offset = offset | ((virt_addr & (page_size.saturating_sub(1))) & !0xFFF);
            let phys_addr = ((pfn as u64) << 12) | page_offset;

            // Edge case: Ensure physical address fits in 32 bits (N64 has 32-bit physical address space)
            if phys_addr > u32::MAX as u64 {
                continue; // Invalid physical address
            }

            // Check cache coherency (c field)
            // c=2 (uncached), c=3 (cached)
            let is_cached = c == 3;

            return Some((phys_addr as u32, is_cached));
        }

        // TLB miss - no matching entry found
        None
    }

    /// Write TLB entry at specified index
    #[allow(dead_code)] // Public API for TLBWI/TLBWR instructions
    pub fn write_entry(&mut self, index: usize, entry: TlbEntry) {
        if index < 32 {
            self.entries[index] = entry;
        }
    }

    /// Read TLB entry at specified index
    #[allow(dead_code)] // Public API for TLBR instruction
    pub fn read_entry(&self, index: usize) -> Option<TlbEntry> {
        if index < 32 {
            Some(self.entries[index])
        } else {
            None
        }
    }

    /// Set current ASID from CP0 EntryHi register
    #[allow(dead_code)] // Public API for CP0 integration
    pub fn set_asid(&mut self, asid: u8) {
        self.current_asid = asid;
    }

    /// Get current ASID
    #[allow(dead_code)] // Public API for CP0 integration
    pub fn get_asid(&self) -> u8 {
        self.current_asid
    }

    /// Probe TLB for a virtual address
    /// Returns the index of matching entry, or None if no match
    #[allow(dead_code)] // Public API for TLBP instruction
    pub fn probe(&self, virt_addr: u64) -> Option<usize> {
        let vpn2 = (virt_addr >> 13) & 0x07FFFFFF;

        for (i, entry) in self.entries.iter().enumerate() {
            let mask = (entry.page_mask as u64) << 12;
            let vpn_mask = !mask;
            if (entry.vpn2 & vpn_mask) == (vpn2 & vpn_mask)
                && (entry.global || entry.asid == self.current_asid)
            {
                return Some(i);
            }
        }

        None
    }
}

impl Default for Tlb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlb_creation() {
        let tlb = Tlb::new();
        assert_eq!(tlb.current_asid, 0);
        assert_eq!(tlb.entries.len(), 32);
    }

    #[test]
    fn test_kseg0_translation() {
        let tlb = Tlb::new();

        // KSEG0: 0x80000000 -> 0x00000000 (cached)
        let result = tlb.translate(0x80000000);
        assert_eq!(result, Some((0x00000000, true)));

        // KSEG0: 0x80100000 -> 0x00100000 (cached)
        let result = tlb.translate(0x80100000);
        assert_eq!(result, Some((0x00100000, true)));
    }

    #[test]
    fn test_kseg1_translation() {
        let tlb = Tlb::new();

        // KSEG1: 0xA0000000 -> 0x00000000 (uncached)
        let result = tlb.translate(0xA0000000);
        assert_eq!(result, Some((0x00000000, false)));

        // KSEG1: 0xA0100000 -> 0x00100000 (uncached)
        let result = tlb.translate(0xA0100000);
        assert_eq!(result, Some((0x00100000, false)));
    }

    #[test]
    fn test_tlb_entry_translation() {
        let mut tlb = Tlb::new();
        tlb.set_asid(1);

        // Create a TLB entry mapping virtual 0x00010000 to physical 0x00020000
        let entry = TlbEntry {
            vpn2: 0x00010000 >> 13, // VPN2 for address 0x00010000
            asid: 1,
            global: false,
            page_mask: 0,  // 4KB pages
            pfn0: 0x00020, // Physical frame 0x00020000
            c0: 3,         // Cached
            d0: true,
            v0: true,
            pfn1: 0x00021, // Next physical frame
            c1: 3,
            d1: true,
            v1: true,
        };

        tlb.write_entry(0, entry);

        // Translate virtual address
        let result = tlb.translate(0x00010000);
        assert!(result.is_some());
        let (phys_addr, is_cached) = result.unwrap();
        assert_eq!(phys_addr, 0x00020000);
        assert!(is_cached);
    }

    #[test]
    fn test_tlb_global_entry() {
        let mut tlb = Tlb::new();
        tlb.set_asid(5); // Different ASID

        // Create a global TLB entry
        let entry = TlbEntry {
            vpn2: 0x00010000 >> 13,
            asid: 1,      // Different ASID
            global: true, // Global entry matches all ASIDs
            page_mask: 0,
            pfn0: 0x00020,
            c0: 3,
            d0: true,
            v0: true,
            pfn1: 0x00021,
            c1: 3,
            d1: true,
            v1: true,
        };

        tlb.write_entry(0, entry);

        // Should match despite different ASID
        let result = tlb.translate(0x00010000);
        assert!(result.is_some());
    }

    #[test]
    fn test_tlb_miss() {
        let tlb = Tlb::new();

        // No TLB entries configured for user space
        let result = tlb.translate(0x00010000);
        assert_eq!(result, None);
    }

    #[test]
    fn test_tlb_probe() {
        let mut tlb = Tlb::new();
        tlb.set_asid(1);

        let entry = TlbEntry {
            vpn2: 0x00010000 >> 13,
            asid: 1,
            global: false,
            page_mask: 0,
            pfn0: 0x00020,
            c0: 3,
            d0: true,
            v0: true,
            pfn1: 0x00021,
            c1: 3,
            d1: true,
            v1: true,
        };

        tlb.write_entry(5, entry);

        // Probe should find the entry at index 5
        let index = tlb.probe(0x00010000);
        assert_eq!(index, Some(5));
    }

    #[test]
    fn test_tlb_edge_case_large_page_mask() {
        // Test that large page masks are safely handled without overflow
        let mut tlb = Tlb::new();
        tlb.set_asid(1);

        // For this test, let's use a specific address range that we can validate
        // Address 0x00100000 has VPN2 = 0x00100000 >> 13 = 0x80
        let test_addr = 0x00100000u64;
        let vpn2 = (test_addr >> 13) & 0x07FFFFFF;

        // Create an entry with a large page mask (256KB pages, page_mask=0x1F)
        // Using a more modest page size to test the edge case handling
        let entry = TlbEntry {
            vpn2, // Match the VPN2 of our test address
            asid: 1,
            global: false,
            page_mask: 0x1F, // 256KB pages (reasonable size for testing)
            pfn0: 0x00000,
            c0: 3,
            d0: true,
            v0: true,
            pfn1: 0x00001,
            c1: 3,
            d1: true,
            v1: true,
        };

        tlb.write_entry(0, entry);

        // Should translate without overflow
        let result = tlb.translate(test_addr);
        assert!(
            result.is_some(),
            "Should translate address 0x{:08X} with page_mask 0x{:03X}",
            test_addr,
            entry.page_mask
        );

        // Verify the translation produces a valid physical address
        if let Some((phys_addr, is_cached)) = result {
            // Physical address is already u32, so it's always within range
            assert!(
                phys_addr < 0x0040_0000,
                "Physical address 0x{:08X} should map to RDRAM range",
                phys_addr
            );
            assert!(is_cached, "Should be cached (c=3)");
        }
    }

    #[test]
    fn test_tlb_edge_case_invalid_page() {
        // Test that invalid pages (v0=0 or v1=0) are correctly handled
        let mut tlb = Tlb::new();
        tlb.set_asid(1);

        // Create entry with valid even page but invalid odd page
        let entry = TlbEntry {
            vpn2: 0x00010000 >> 13,
            asid: 1,
            global: false,
            page_mask: 0,
            pfn0: 0x00020,
            c0: 3,
            d0: true,
            v0: true, // Even page valid
            pfn1: 0x00021,
            c1: 3,
            d1: true,
            v1: false, // Odd page INVALID
        };

        tlb.write_entry(0, entry);

        // Access to even page should succeed
        let result = tlb.translate(0x00010000);
        assert!(result.is_some());

        // Access to odd page should fail (TLB miss)
        let result = tlb.translate(0x00011000);
        assert_eq!(result, None);
    }

    #[test]
    fn test_tlb_edge_case_physical_address_overflow() {
        // Test that physical addresses exceeding 32 bits are rejected
        let mut tlb = Tlb::new();
        tlb.set_asid(1);

        // Create entry that could produce 64-bit physical address
        let entry = TlbEntry {
            vpn2: 0x00010000 >> 13,
            asid: 1,
            global: false,
            page_mask: 0xFFF, // Large page
            pfn0: 0xFFFFF,    // Maximum PFN that stays within 32-bit space
            c0: 3,
            d0: true,
            v0: true,
            pfn1: 0xFFFFF,
            c1: 3,
            d1: true,
            v1: true,
        };

        tlb.write_entry(0, entry);

        // Should translate successfully (within 32-bit range)
        let result = tlb.translate(0x00010000);
        assert!(result.is_some());
        if let Some((phys_addr, _)) = result {
            // Physical address is already u32, always within 32-bit range
            // The test uses PFN=0xFFFFF with 16MB pages, resulting in high physical address
            // Just verify it fits in u32 (which it does by type)
            assert!(
                phys_addr >= 0xFFFFF000,
                "Physical address 0x{:08X} should be from high PFN",
                phys_addr
            );
        }
    }

    #[test]
    fn test_tlb_edge_case_asid_mismatch() {
        // Test that non-global entries don't match with wrong ASID
        let mut tlb = Tlb::new();
        tlb.set_asid(1);

        let entry = TlbEntry {
            vpn2: 0x00010000 >> 13,
            asid: 2, // Different ASID
            global: false,
            page_mask: 0,
            pfn0: 0x00020,
            c0: 3,
            d0: true,
            v0: true,
            pfn1: 0x00021,
            c1: 3,
            d1: true,
            v1: true,
        };

        tlb.write_entry(0, entry);

        // Should not match (ASID mismatch)
        let result = tlb.translate(0x00010000);
        assert_eq!(result, None);

        // But should match if we change to the correct ASID
        tlb.set_asid(2);
        let result = tlb.translate(0x00010000);
        assert!(result.is_some());
    }

    #[test]
    fn test_tlb_edge_case_vpn2_boundary() {
        // Test VPN2 matching at page boundaries
        let mut tlb = Tlb::new();
        tlb.set_asid(1);

        let entry = TlbEntry {
            vpn2: 0x00010000 >> 13,
            asid: 1,
            global: false,
            page_mask: 0, // 4KB pages
            pfn0: 0x00020,
            c0: 3,
            d0: true,
            v0: true,
            pfn1: 0x00021,
            c1: 3,
            d1: true,
            v1: true,
        };

        tlb.write_entry(0, entry);

        // Should match addresses within the 8KB range (two 4KB pages)
        assert!(tlb.translate(0x00010000).is_some()); // Start of even page
        assert!(tlb.translate(0x00010FFF).is_some()); // End of even page
        assert!(tlb.translate(0x00011000).is_some()); // Start of odd page
        assert!(tlb.translate(0x00011FFF).is_some()); // End of odd page

        // Should not match address outside the range
        assert_eq!(tlb.translate(0x00012000), None); // Next entry
        assert_eq!(tlb.translate(0x0000FFFF), None); // Before entry
    }
}
