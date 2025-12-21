; Simple NES test ROM
; This ROM writes a known pattern to the PPU to verify basic functionality
; Pattern: Fills screen with tile $55 (checkerboard pattern)

.segment "HEADER"
    ; iNES header
    .byte "NES", $1A        ; Magic string
    .byte 1                 ; 1 x 16KB PRG-ROM
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
    
    ; Wait for PPU warmup
    bit $2002
:   bit $2002
    bpl :-
:   bit $2002
    bpl :-
    
    ; Initialize PPU
    lda #$00
    sta $2000               ; PPU control - NMI off
    sta $2001               ; PPU mask - rendering off
    
    ; Set palette
    lda #$3F
    sta $2006               ; PPU address high
    lda #$00
    sta $2006               ; PPU address low
    
    ; Write simple palette (black, white, gray, light gray)
    lda #$0F                ; Black
    sta $2007
    lda #$30                ; White
    sta $2007
    lda #$00                ; Dark gray
    sta $2007
    lda #$10                ; Light gray
    sta $2007
    
    ; Fill nametable with test pattern (tile $55)
    lda #$20
    sta $2006               ; PPU address high (nametable 0)
    lda #$00
    sta $2006               ; PPU address low
    
    ldx #$00
    ldy #$00
fill_loop:
    lda #$55                ; Test pattern tile
    sta $2007
    inx
    cpx #$00
    bne fill_loop
    iny
    cpy #$04                ; 4 * 256 = 1024 bytes (full nametable)
    bne fill_loop
    
    ; Reset scroll
    lda #$00
    sta $2005
    sta $2005
    
    ; Enable rendering
    lda #%00011110          ; Show background and sprites
    sta $2001
    lda #%10000000          ; Enable NMI
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
    ; Tile $00: Blank (all zeros)
    .res 16, $00
    
    ; Tiles $01-$54: Random patterns (not used in this test)
    .res 16 * 84, $00
    
    ; Tile $55: Checkerboard pattern
    .byte $AA, $00, $55, $00, $AA, $00, $55, $00
    .byte $00, $AA, $00, $55, $00, $AA, $00, $55
    
    ; Fill rest of CHR-ROM
    .res 8192 - 16 - (16 * 84) - 16, $00
