; SNES test ROM - displays a simple pattern
; Purpose: Verify basic PPU functionality (palette, tilemap, CHR rendering)

.p816                       ; 65816 processor
.a8                         ; 8-bit accumulator by default
.i8                         ; 8-bit index registers by default

.segment "HEADER"
    ; SNES ROM header (internal header at $FFB0-$FFDF for LoROM)
    ; Title (21 bytes)
    .byte "SNES TEST ROM        "
    
    ; ROM makeup byte
    .byte $20               ; LoROM, slow speed
    
    ; ROM type (ROM only, no RAM or special chips)
    .byte $00
    
    ; ROM size (32KB = $08)
    .byte $08
    
    ; RAM size (no RAM)
    .byte $00
    
    ; Country code (01 = USA)
    .byte $01
    
    ; Developer ID (33 = Extended header)
    .byte $33
    
    ; Version number
    .byte $00
    
    ; Checksum complement
    .word $0000
    
    ; Checksum
    .word $0000
    
.segment "VECTORS"
    ; Native mode vectors ($FFE0-$FFEF)
    .word $0000             ; $FFE0 - unused
    .word $0000             ; $FFE2 - unused
    .word NMI               ; $FFE4 - COP (reuse NMI)
    .word $0000             ; $FFE6 - BRK (unused)
    .word $0000             ; $FFE8 - ABORT (unused)
    .word NMI               ; $FFEA - NMI
    .word $0000             ; $FFEC - reserved
    .word IRQ               ; $FFEE - IRQ
    
    ; Emulation mode vectors ($FFF0-$FFFF)
    .word $0000             ; $FFF0 - unused
    .word $0000             ; $FFF2 - unused  
    .word NMI               ; $FFF4 - COP (reuse NMI)
    .word $0000             ; $FFF6 - reserved
    .word $0000             ; $FFF8 - ABORT (unused)
    .word NMI               ; $FFFA - NMI
    .word RESET             ; $FFFC - RESET (entry point!)
    .word IRQ               ; $FFFE - IRQ/BRK

.segment "CODE"

RESET:
    sei                     ; Disable interrupts
    clc
    xce                     ; Switch to native mode (clear emulation flag)
    
    rep #$10                ; 16-bit index registers
    .i16
    sep #$20                ; 8-bit accumulator
    .a8
    
    ; Set up stack
    ldx #$1FFF
    txs
    
    ; Wait for VBlank
    lda $4212
:   lda $4212
    and #$80
    beq :-
    
    ; Turn off screen
    lda #$80
    sta $2100               ; Force blank
    
    ; Set up PPU registers for Mode 0 (4-color BG mode)
    lda #$00
    sta $2105               ; BG mode 0
    
    ; Set BG1 tilemap to VRAM $0000, size 32x32
    lda #$00
    sta $2107               ; BG1 tilemap address
    
    ; Set BG1 CHR to VRAM $1000  
    lda #$01
    sta $210B               ; BG1 CHR address
    
    ; Enable BG1 on main screen
    lda #$01
    sta $212C               ; Main screen designation
    
    ; Set up palette - simple 4-color palette for testing
    stz $2121               ; CGRAM address = 0
    
    ; Color 0: Black (backdrop)
    stz $2122
    stz $2122
    
    ; Color 1: White
    lda #$FF
    sta $2122
    lda #$7F
    sta $2122
    
    ; Color 2: Red
    lda #$1F
    sta $2122
    stz $2122
    
    ; Color 3: Blue
    stz $2122
    lda #$7C
    sta $2122
    
    ; Upload tile data to VRAM
    ; Set VRAM address to $1000 (CHR data for BG1)
    ldx #$1000
    stx $2116               ; VRAM address
    
    ; Tile 0: White square (all pixels use color 1)
    ; Each tile is 8x8 pixels, 2 bits per pixel in planar format
    ; For Mode 0, we have bitplanes 0 and 1
    lda #$FF
    ldx #$0010              ; 16 bytes per tile (8 rows * 2 bitplanes)
:   sta $2118               ; Write to VRAM data port
    dex
    bne :-
    
    ; Tile 1: Red square (pattern 10 in binary = color 2)
    stz $2118               ; Bitplane 0: all 0s (8 bytes)
    stz $2118
    stz $2118
    stz $2118
    stz $2118
    stz $2118
    stz $2118
    stz $2118
    
    lda #$FF                ; Bitplane 1: all 1s (8 bytes)
    sta $2118
    sta $2118
    sta $2118
    sta $2118
    sta $2118
    sta $2118
    sta $2118
    sta $2118
    
    ; Upload tilemap to VRAM
    ; Set VRAM address to $0000 (tilemap for BG1)
    ldx #$0000
    stx $2116
    
    ; Fill 32x32 tilemap with checkerboard pattern (tiles 0 and 1)
    ldy #$0400              ; 1024 tiles (32x32)
    ldx #$0000              ; Start with tile 0
tilemap_loop:
    txa
    sta $2118               ; Write tile number to VRAM
    stz $2119               ; Write high byte (attributes) = 0
    
    ; Toggle between 0 and 1
    txa
    eor #$01
    tax
    
    dey
    bne tilemap_loop
    
    ; Turn on screen (brightness = 15)
    lda #$0F
    sta $2100
    
forever:
    wai                     ; Wait for interrupt
    jmp forever

NMI:
    rti

IRQ:
    rti
