; Simple Game Boy test ROM
; This ROM writes a known pattern to VRAM to verify basic functionality

SECTION "Header", ROM0[$100]
    ; Entry point
    nop
    jp Start

    ; Nintendo logo (required for valid GB ROM)
    DB $CE,$ED,$66,$66,$CC,$0D,$00,$0B,$03,$73,$00,$83,$00,$0C,$00,$0D
    DB $00,$08,$11,$1F,$88,$89,$00,$0E,$DC,$CC,$6E,$E6,$DD,$DD,$D9,$99
    DB $BB,$BB,$67,$63,$6E,$0E,$EC,$CC,$DD,$DC,$99,$9F,$BB,$B9,$33,$3E

    ; Title (max 16 bytes)
    DB "GBTEST"
    DS $143 - @, 0

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
    
    ; Clear VRAM
    ld hl, $8000
    ld bc, $2000
.clearVRAM:
    ld [hl+], a
    dec bc
    ld a, b
    or c
    jr nz, .clearVRAM
    
    ; Write test pattern to tile data ($8000-$8010)
    ; Tile $00: Checkerboard pattern
    ld hl, $8000
    ld a, $AA
    ld [hl+], a
    ld a, $00
    ld [hl+], a
    ld a, $55
    ld [hl+], a
    ld a, $00
    ld [hl+], a
    ld a, $AA
    ld [hl+], a
    ld a, $00
    ld [hl+], a
    ld a, $55
    ld [hl+], a
    ld a, $00
    ld [hl+], a
    ld a, $00
    ld [hl+], a
    ld a, $AA
    ld [hl+], a
    ld a, $00
    ld [hl+], a
    ld a, $55
    ld [hl+], a
    ld a, $00
    ld [hl+], a
    ld a, $AA
    ld [hl+], a
    ld a, $00
    ld [hl+], a
    ld a, $55
    
    ; Fill tilemap with tile $00
    ld hl, $9800
    ld bc, $0400           ; 1024 bytes
.fillTilemap:
    xor a                  ; Tile $00
    ld [hl+], a
    dec bc
    ld a, b
    or c
    jr nz, .fillTilemap
    
    ; Set palette (DMG only - all black to white gradient)
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
