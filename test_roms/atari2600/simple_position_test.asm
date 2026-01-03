; Simple Atari 2600 Position Test ROM
; Draws two sprites: one in upper-right, one in center
; Sprites are always visible to make testing easier

    processor 6502
    include "vcs.h"

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
    
    ; Set up player graphics (8x8 filled square)
    lda #%11111111
    sta GRP0
    sta GRP1
    
    ; Set player colors
    lda #$0E        ; White for both players
    sta COLUP0
    sta COLUP1
    
    ; Set playfield color
    lda #$0E        ; White
    sta COLUPF
    
    ; Set background color
    lda #$00        ; Black
    sta COLUBK

MainLoop:
    ;===================
    ; VSYNC (3 scanlines)
    ;===================
    lda #2
    sta VSYNC
    sta WSYNC
    sta WSYNC
    sta WSYNC
    lda #0
    sta VSYNC
    
    ;===================
    ; VBLANK (37 scanlines)
    ;===================
    lda #$02
    sta VBLANK
    
    ; Clear any previous positioning
    sta WSYNC
    sta HMCLR
    
    ; Position Player 0 at X=130 (upper right)
    ldx #130
    jsr PositionPlayer0
    
    ; Position Player 1 at X=76 (center)
    ldx #76
    jsr PositionPlayer1
    
    ; Remaining VBLANK
    ldx #30
VBlankLoop:
    sta WSYNC
    dex
    bne VBlankLoop
    
    ; Turn off VBLANK
    lda #0
    sta VBLANK
    
    ;===================
    ; Visible Screen (192 scanlines)
    ;===================
    ldx #192
ScreenLoop:
    sta WSYNC
    dex
    bne ScreenLoop
    
    ;===================
    ; Overscan (30 scanlines)
    ;===================
    lda #$02
    sta VBLANK
    
    ldx #30
OverscanLoop:
    sta WSYNC
    dex
    bne OverscanLoop
    
    jmp MainLoop

;===================
; Position Player 0
; X register contains desired position (0-159)
;===================
PositionPlayer0:
    sta WSYNC       ; Start of scanline
PositionP0Loop:
    dex
    bpl PositionP0Loop
    sta RESP0       ; Reset position at desired X
    rts

;===================
; Position Player 1
; X register contains desired position (0-159)
;===================
PositionPlayer1:
    sta WSYNC       ; Start of scanline
PositionP1Loop:
    dex
    bpl PositionP1Loop
    sta RESP1       ; Reset position at desired X
    rts

    ; Interrupt vectors
    org $FFFC
    .word Start     ; RESET
    .word Start     ; IRQ/BRK (not used)
