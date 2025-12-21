; Simple Atari 2600 test ROM
; This ROM sets up the TIA to display a known pattern

    processor 6502
    include "vcs.h"

    seg.u Variables
    org $80

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
    lda #$0E        ; White
    sta COLUPF
    
    ; Set background color to black
    lda #$00        ; Black
    sta COLUBK
    
    ; Set playfield pattern (simple pattern)
    lda #$AA        ; Alternating pattern
    sta PF0
    sta PF1
    sta PF2
    
MainLoop:
    ; Wait for vertical blank
    lda #2
    sta VSYNC
    
    ; Generate VSYNC signal (3 scanlines)
    sta WSYNC
    sta WSYNC
    sta WSYNC
    
    lda #0
    sta VSYNC
    
    ; VBLANK period (37 scanlines)
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
    
    ; Visible screen (192 scanlines)
    ldx #192
ScreenLoop:
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
