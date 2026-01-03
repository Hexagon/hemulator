; SNES Sprite Overflow Test ROM
; Purpose: Test sprite-per-scanline limits (32 sprites, 34 tile slots)
; Expected: First 32 sprites on a scanline should render, rest should be dropped

.p816                       ; 65816 processor
.a8                         ; 8-bit accumulator by default
.i8                         ; 8-bit index registers by default

.segment "HEADER"
    ; SNES ROM header (internal header at $FFB0-$FFDF for LoROM)
    ; Title (21 bytes)
    .byte "SPRITE OVERFLOW TEST "
    
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
    
    ; Enable force blank (allows VRAM access)
    lda #$80
    sta $2100
    
    ; Set Mode 0 (4 BG layers, 2bpp each)
    stz $2105               ; BGMODE = Mode 0
    
    ; Configure sprite settings
    ; Object size: 8x8 and 16x16
    lda #$02                ; Small = 8x8, Large = 16x16, Name base = 0, Name select = 0
    sta $2101               ; OBSEL register
    
    ; Set VRAM address to $0000 (sprite CHR data)
    stz $2116               ; Low byte
    stz $2117               ; High byte
    
    ; Upload sprite tile data (one simple 8x8 sprite)
    ldx #$0000
:   lda SpriteData, x
    sta $2118               ; Write low byte
    inx
    cpx #$0020              ; 32 bytes (1 tile * 32 bytes for 4bpp)
    bne :-
    
    ; Set up palette for sprites (colors 128-255)
    lda #$80                ; Start at color 128 (sprite palette 0)
    sta $2121
    
    ; Color 0: Transparent
    stz $2122
    stz $2122
    
    ; Color 1: White
    lda #$FF
    sta $2122
    lda #$7F
    sta $2122
    
    ; Set OAM address to 0
    stz $2102
    stz $2103
    
    ; Fill OAM with 128 sprites, all at Y=100 (to create overflow on that scanline)
    ; This tests the 32 sprite per scanline limit
    rep #$20                ; 16-bit accumulator
    .a16
    ldx #$0000
SpriteLoop:
    txa
    and #$00FF              ; X position = sprite index (0-127)
    sta $2104               ; X position (low byte) and Y position
    
    sep #$20                ; Back to 8-bit
    .a8
    lda #100                ; Y position = 100 (all on same scanline!)
    sta $2104
    
    lda #$00                ; Tile number 0
    sta $2104
    
    lda #$00                ; Palette 0, priority 0, no flip
    sta $2104
    
    rep #$20
    .a16
    inx
    cpx #$0080              ; 128 sprites
    bne SpriteLoop
    
    sep #$20                ; Back to 8-bit
    .a8
    
    ; Fill high table (bits 8 of X position and size bit)
    ldx #$0000
HighTableLoop:
    stz $2104               ; X MSB = 0, size = small (8x8)
    inx
    cpx #$0020              ; 32 bytes (128 sprites / 4)
    bne HighTableLoop
    
    ; Enable sprites on main screen
    lda #$10
    sta $212C               ; Enable sprites
    
    ; Disable force blank (show screen)
    lda #$0F                ; Full brightness
    sta $2100
    
    ; Enable NMI
    lda #$80
    sta $4200
    
    ; Main loop
    cli
MainLoop:
    wai
    jmp MainLoop

NMI:
    rti

IRQ:
    rti

; Sprite tile data (1 tile, 4bpp, 32 bytes)
SpriteData:
    ; 8 rows of bitplane 0 and 1 interleaved
    .byte $FF, $00, $FF, $00, $FF, $00, $FF, $00
    .byte $FF, $00, $FF, $00, $FF, $00, $FF, $00
    ; 8 rows of bitplane 2 and 3 interleaved
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00
