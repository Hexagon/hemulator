; Atari 2600 Checkerboard Test ROM
; This ROM creates a true checkerboard pattern using playfield registers
; The pattern alternates every 2 scanlines to create a vertical checkerboard effect

    processor 6502
    include "vcs.h"

    seg.u Variables
    org $80

ScanlineCounter ds 1    ; Current scanline in visible area

    seg Code
    org $F000

Start:
    sei             ; Disable interrupts
    cld             ; Clear decimal mode
    ldx #$FF
    txs             ; Set up stack
    
    ; Clear RAM and TIA
    lda #0
ClearMem:
    sta $00,x
    dex
    bne ClearMem
    
    ; Set playfield color to white
    lda #$0E        ; White (high luminance)
    sta COLUPF
    
    ; Set background color to black
    lda #$00        ; Black
    sta COLUBK
    
MainLoop:
    ; VSYNC (3 scanlines)
    lda #2
    sta VSYNC
    sta WSYNC
    sta WSYNC
    sta WSYNC
    lda #0
    sta VSYNC
    
    ; VBLANK (37 scanlines)
    lda #$02
    sta VBLANK
    
    ldx #37
VBlankLoop:
    sta WSYNC
    dex
    bne VBlankLoop
    
    ; Turn off VBLANK
    lda #0
    sta VBLANK
    sta ScanlineCounter
    
    ; Visible screen (192 scanlines)
    ; Create checkerboard by alternating playfield pattern every 2 scanlines
    ldx #96         ; 192 scanlines / 2 = 96 pairs
ScreenLoop:
    ; First scanline of pair - use 0xAA pattern (10101010)
    lda #$AA
    sta PF0
    sta PF1
    sta PF2
    sta WSYNC
    
    ; Second scanline of pair - use 0x55 pattern (01010101)
    lda #$55
    sta PF0
    sta PF1
    sta PF2
    sta WSYNC
    
    dex
    bne ScreenLoop
    
    ; Overscan (30 scanlines)
    lda #$02
    sta VBLANK
    
    ldx #30
OverscanLoop:
    sta WSYNC
    dex
    bne OverscanLoop
    
    jmp MainLoop

    ; Interrupt vectors
    org $FFFC
    .word Start     ; RESET
    .word Start     ; IRQ/BRK (not used)
