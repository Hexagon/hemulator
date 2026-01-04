; Atari 2600 Position Test ROM
; Tests accurate sprite and playfield positioning
; This ROM displays:
; - A sprite in the upper right corner
; - A sprite in the center
; - A playfield section in the center
; Used to verify that TIA positioning works correctly

    processor 6502
    include "vcs.h"

    seg.u Variables
    org $80

ScanlineCount ds 1      ; Current scanline in visible area

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
    lda #$0E        ; White for player 0 (upper right)
    sta COLUP0
    lda #$0E        ; White for player 1 (center)
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
    
    ; Position Player 0 at X=130 (upper right area)
    ; Use standard Atari positioning technique
    ldx #130
    jsr PositionPlayer0
    
    ; Position Player 1 at X=76 (center)
    ldx #76
    jsr PositionPlayer1
    
    ; Continue VBLANK
    ldx #30
VBlankLoop:
    sta WSYNC
    dex
    bne VBlankLoop
    
    ; Turn off VBLANK
    lda #0
    sta VBLANK
    sta ScanlineCount
    
    ;===================
    ; Visible Screen (192 scanlines)
    ;===================
    
    ; Section 1: Upper area (scanlines 0-39)
    ; Player 0 should be visible at scanlines 20-27 (8 scanlines for 8-pixel sprite)
    ldx #40
UpperSection:
    sta WSYNC
    
    ; Check if we should show player 0
    lda ScanlineCount
    cmp #20
    bcc NoPlayer0
    cmp #28
    bcs NoPlayer0
    ; Player 0 is visible
    lda #%11111111
    sta GRP0
    jmp CheckPlayer1Upper
NoPlayer0:
    lda #0
    sta GRP0
    
CheckPlayer1Upper:
    ; Player 1 not visible in upper section
    lda #0
    sta GRP1
    
    ; No playfield in upper section
    lda #0
    sta PF0
    sta PF1
    sta PF2
    
    inc ScanlineCount
    dex
    bne UpperSection
    
    ; Section 2: Middle area (scanlines 40-151)
    ; Player 1 should be visible at scanlines 96-103 (center)
    ; Also show a playfield block in center
    ldx #112
MiddleSection:
    sta WSYNC
    
    ; Player 0 not visible in middle
    lda #0
    sta GRP0
    
    ; Check if we should show player 1
    lda ScanlineCount
    cmp #92
    bcc NoPlayer1Mid
    cmp #100
    bcs NoPlayer1Mid
    ; Player 1 is visible (center sprite)
    lda #%11111111
    sta GRP1
    jmp CheckPlayfield
NoPlayer1Mid:
    lda #0
    sta GRP1
    
CheckPlayfield:
    ; Show playfield in center area (scanlines 80-110)
    lda ScanlineCount
    cmp #80
    bcc NoPlayfield
    cmp #111
    bcs NoPlayfield
    ; Show playfield
    lda #$F0
    sta PF0
    lda #$FF
    sta PF1
    sta PF2
    jmp DonePlayfield
NoPlayfield:
    lda #0
    sta PF0
    sta PF1
    sta PF2
    
DonePlayfield:
    inc ScanlineCount
    dex
    bne MiddleSection
    
    ; Section 3: Lower area (scanlines 152-191)
    ldx #40
LowerSection:
    sta WSYNC
    
    ; No sprites in lower section
    lda #0
    sta GRP0
    sta GRP1
    sta PF0
    sta PF1
    sta PF2
    
    inc ScanlineCount
    dex
    bne LowerSection
    
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
