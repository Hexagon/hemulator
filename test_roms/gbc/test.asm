; Simple Game Boy Color test ROM
; This ROM writes a known pattern to VRAM to verify basic functionality

SECTION "Header", ROM0[$100]
    ; Entry point
    nop
    jp Start

    ; Nintendo logo (required for valid GB ROM)
    DB $CE,$ED,$66,$66,$CC,$0D,$00,$0B,$03,$73,$00,$83,$00,$0C,$00,$0D
    DB $00,$08,$11,$1F,$88,$89,$00,$0E,$DC,$CC,$6E,$E6,$DD,$DD,$D9,$99
    DB $BB,$BB,$67,$63,$6E,$0E,$EC,$CC,$DD,$DC,$99,$9F,$BB,$B9,$33,$3E

    ; Title (max 15 bytes for CGB)
    DB "GBCTEST"
    DS $13F - @, 0
    
    ; Manufacturer code (0x13F-0x142)
    DB $00,$00,$00,$00
    
    ; CGB flag: $80 = CGB + DMG compatible, $C0 = CGB only
    DB $80
    
    ; Licensee code
    DB $00,$00
    
    ; SGB flag: $00 = No SGB features
    DB $00
    
    ; Cartridge type: $00 = ROM only
    DB $00
    
    ; ROM size: $00 = 32KB
    DB $00
    
    ; RAM size: $00 = No RAM
    DB $00
    
    ; Region: $01 = Non-Japanese
    DB $01
    
    ; Licensee: $00 = None
    DB $00
    
    ; Version: $00
    DB $00
    
    ; Header checksum (will be fixed by rgbfix)
    DB $00
    
    ; Global checksum (will be fixed by rgbfix)
    DW $0000

SECTION "Code", ROM0[$150]

Start:
    di                      ; Disable interrupts
    
    ; Wait for VBlank
.waitVBlank:
    ldh a, [$FF44]         ; Read LY register
    cp 144
    jr c, .waitVBlank
    
    ; Disable LCD
    xor a
    ldh [$FF40], a         ; LCDC = 0
    
    ; Tile 0: White (color index 0 = bitplane0=0, bitplane1=0)
    ld hl, $8000
    xor a                  ; a = 0
    ld b, 16
.t0:
    ld [hl+], a            ; Fill 16 bytes with 0
    dec b
    jr nz, .t0
    
    ; Tile 1: Dark (color index 3 = bitplane0=1, bitplane1=1)
    ; We need both bitplanes set to 0xFF for full dark/black
    ld hl, $8010
    ld a, $FF
    ld b, 16
.t1:
    ld [hl+], a            ; Fill 16 bytes with $FF (both bitplanes)
    dec b
    jr nz, .t1
    
    ; Fill tilemap: alternating 0 and 1 in checkerboard pattern
    ld hl, $9800
    ld c, 18               ; rows (Full screen is 18 tiles vertically)
.row:
    ld b, 20               ; cols (Full screen is 20 tiles horizontally)
    ld a, c
    and 1                  ; Start pattern based on row number (ensures checkerboard)
.col:
    ld [hl+], a
    xor 1                  ; Toggle between 0 and 1
    dec b
    jr nz, .col
    dec c
    jr nz, .row
    
    ; Set palette (DMG-compatible mode - works on both DMG and CGB)
    ld a, %11100100        ; 3=black, 2=dark gray, 1=light gray, 0=white
    ldh [$FF47], a         ; BGP
    
    ; Set scroll to 0
    xor a
    ldh [$FF42], a         ; SCY
    ldh [$FF43], a         ; SCX
    
    ; Enable LCD with background
    ld a, %10000001        ; LCD on, BG on, use tilemap $9800
    ldh [$FF40], a
    
.forever:
    halt
    jr .forever
