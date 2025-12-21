; NES test ROM - displays a checkerboard pattern
; Purpose: Verify basic PPU functionality (palette, tilemap, CHR rendering)

.segment "HEADER"
    ; iNES header
    .byte "NES", $1A        ; Magic string
    .byte 2                 ; 2 x 16KB PRG-ROM
    .byte 1                 ; 1 x 8KB CHR-ROM
    .byte $00               ; Mapper 0 (NROM), horizontal mirroring
    .byte $00               ; No special features
    .res 8, 0               ; Padding

.segment "VECTORS"
    .word NMI
    .word RESET
    .word IRQ

.segment "STARTUP"

.segment "CODE"

RESET:
    sei                     ; Disable interrupts
    cld                     ; Clear decimal mode
    ldx #$FF
    txs                     ; Set up stack
    
    ; Wait for PPU warmup (2 VBlanks)
    bit $2002
:   bit $2002
    bpl :-
:   bit $2002
    bpl :-
    
    ; Initialize PPU control registers
    lda #$00
    sta $2000               ; PPU control - NMI off, pattern table 0
    sta $2001               ; PPU mask - rendering off
    
    ; Set up palette - use bright distinct colors for testing
    lda #$3F
    sta $2006               ; PPU address high
    lda #$00
    sta $2006               ; PPU address low
    
    ; Write BG palette 0 (used by our tiles)
    lda #$0F                ; Universal background: Black
    sta $2007
    lda #$30                ; Color 1: White
    sta $2007
    lda #$16                ; Color 2: Red  
    sta $2007
    lda #$11                ; Color 3: Blue
    sta $2007
    
    ; Fill rest of palettes with black
    ldx #28                 ; 7 more palettes * 4 colors - 4 already written
:   lda #$0F
    sta $2007
    dex
    bne :-
    
    ; Fill nametable with alternating tiles to create checkerboard
    lda #$20
    sta $2006               ; PPU address high (nametable 0)
    lda #$00
    sta $2006               ; PPU address low
    
    ; Fill 960 tiles (30 rows * 32 columns) with alternating pattern
    ldx #30                 ; 30 rows
row_loop:
    ldy #32                 ; 32 columns
col_loop:
    ; Alternate between tile 0 and tile 1
    txa                     ; Row number in X
    eor #$FF                ; Invert for alternating rows
    and #$01                ; Get bit 0
    sta $2007               ; Write tile index (0 or 1)
    dey
    bne col_loop
    dex
    bne row_loop
    
    ; Clear attribute table (use palette 0 for all tiles)
    ldx #64
:   lda #$00
    sta $2007
    dex
    bne :-
    
    ; Reset scroll position
    lda #$00
    sta $2005
    sta $2005
    
    ; Enable rendering (no grayscale, show background)
    lda #$0A                ; Bits: show BG, no sprites
    sta $2001
    lda #$80                ; Enable NMI
    sta $2000
    
forever:
    jmp forever

NMI:
    rti

IRQ:
    rti

.segment "RODATA"

.segment "CHR"
    ; CHR-ROM data: 8KB
    
    ; Tile $00: White square (color 1 in palette)
    .byte $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF  ; Low bitplane
    .byte $00, $00, $00, $00, $00, $00, $00, $00  ; High bitplane
    
    ; Tile $01: Red square (color 2 in palette)
    .byte $00, $00, $00, $00, $00, $00, $00, $00  ; Low bitplane  
    .byte $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF  ; High bitplane
    
    ; Fill rest of CHR-ROM with zeros
    .res 8192 - 32, $00
