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
    
    ; Tile 0: Blue square (all pixels use color 3 = binary 11)
    ; Bitplane 0: all 1s (bytes 0-7), Bitplane 1: all 1s (bytes 8-15)
    ; Write as 16-bit words to VRAM
    lda #$FF
    ldx #$0008              ; 16 bytes = 8 words
:   sta $2118               ; Low byte
    sta $2119               ; High byte (same as low for tile 0)
    dex
    bne :-
    
    ; Tile 1: Red square (color 2 = binary 10)
    ; Bitplane 0: all 0s (bytes 0-7), Bitplane 1: all 1s (bytes 8-15)
    ; Write as 16-bit words
    stz $2118               ; Bitplane 0, row 0 (low byte)
    stz $2119               ; Bitplane 0, row 1 (high byte)
    stz $2118               ; Bitplane 0, row 2
    stz $2119               ; Bitplane 0, row 3
    stz $2118               ; Bitplane 0, row 4
    stz $2119               ; Bitplane 0, row 5
    stz $2118               ; Bitplane 0, row 6
    stz $2119               ; Bitplane 0, row 7
    
    lda #$FF
    sta $2118               ; Bitplane 1, row 0 (low byte)
    sta $2119               ; Bitplane 1, row 1 (high byte)
    sta $2118               ; Bitplane 1, row 2
    sta $2119               ; Bitplane 1, row 3
    sta $2118               ; Bitplane 1, row 4
    sta $2119               ; Bitplane 1, row 5
    sta $2118               ; Bitplane 1, row 6
    sta $2119               ; Bitplane 1, row 7
    
    ; Upload tilemap to VRAM
    ; Set VRAM address to $0000 (tilemap for BG1)
    ldx #$0000
    stx $2116
    
    ; Fill 32x32 tilemap with checkerboard pattern (tiles 0 and 1)
    ; We need a 2D checkerboard: (x + y) & 1
    ; Use Y for row counter, X for column counter
    ldy #$0000              ; Y = row (0-31)
row_loop:
    ldx #$0000              ; X = column (0-31)
col_loop:
    ; Calculate checkerboard tile: (x + y) & 1
    ; Save X and Y to temp variables in RAM (use WRAM at $7E0000)
    txa
    sta $7E0000             ; Save X to WRAM
    tya
    sta $7E0001             ; Save Y to WRAM
    
    ; Calculate (X + Y) & 1
    clc
    adc $7E0000             ; A = Y + X
    and #$01                ; A = (Y + X) & 1
    
    sta $2118               ; Write tile number to VRAM
    stz $2119               ; Write high byte (attributes) = 0
    
    ; Restore X from WRAM
    lda $7E0000
    tax
    
    inx                     ; Next column
    cpx #$0020              ; 32 columns?
    bne col_loop
    
    iny                     ; Next row
    cpy #$0020              ; 32 rows?
    bne row_loop
    
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
