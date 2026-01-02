; Atari 2600 Game-Like Test ROM
; This ROM more closely resembles real game behavior to test for common emulation issues:
; - Per-scanline color changes (color bars)
; - Sprite positioning and movement
; - Playfield patterns
; - HMOVE timing
; - VBLANK/VSYNC timing accuracy
;
; Expected behavior:
; - Top section: Color bars that change every 8 scanlines
; - Middle section: Two sprites (players) that move horizontally
; - Bottom section: Playfield pattern
; - Background should be stable (not flickering)

    processor 6502
    include "vcs.h"

    seg.u Variables
    org $80

FrameCount ds 1         ; Frame counter
Player0Pos ds 1         ; Player 0 X position
Player1Pos ds 1         ; Player 1 X position
Player0Dir ds 1         ; Player 0 direction (0=right, 1=left)
Player1Dir ds 1         ; Player 1 direction

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
    
    ; Initialize variables
    lda #0
    sta FrameCount
    lda #40
    sta Player0Pos
    lda #120
    sta Player1Pos
    lda #0
    sta Player0Dir
    sta Player1Dir
    
    ; Set up player graphics (simple 8-pixel sprite)
    lda #%11111111
    sta GRP0
    sta GRP1
    
    ; Set player colors
    lda #$44        ; Blue
    sta COLUP0
    lda #$C4        ; Red
    sta COLUP1
    
    ; Set playfield color
    lda #$0E        ; White
    sta COLUPF

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
    
    ; Update player positions during VBLANK
    ; Move player 0 right/left
    lda Player0Dir
    bne MoveP0Left
    
MoveP0Right:
    inc Player0Pos
    lda Player0Pos
    cmp #150        ; Right edge
    bcc DoneMoveP0
    lda #1
    sta Player0Dir
    jmp DoneMoveP0
    
MoveP0Left:
    dec Player0Pos
    lda Player0Pos
    cmp #10         ; Left edge
    bcs DoneMoveP0
    lda #0
    sta Player0Dir
    
DoneMoveP0:
    ; Move player 1 (opposite direction)
    lda Player1Dir
    bne MoveP1Left
    
MoveP1Right:
    dec Player1Pos  ; Move left (opposite)
    lda Player1Pos
    cmp #10
    bcs DoneMoveP1
    lda #1
    sta Player1Dir
    jmp DoneMoveP1
    
MoveP1Left:
    inc Player1Pos  ; Move right (opposite)
    lda Player1Pos
    cmp #150
    bcc DoneMoveP1
    lda #0
    sta Player1Dir
    
DoneMoveP1:
    ; Position players
    ; Coarse positioning
    ldx Player0Pos
    jsr PositionPlayer0
    
    ldx Player1Pos
    jsr PositionPlayer1
    
    ; Wait for remaining VBLANK scanlines
    ldx #30
VBlankLoop:
    sta WSYNC
    dex
    bne VBlankLoop
    
    ;===================
    ; Visible screen (192 scanlines)
    ;===================
    ; Turn off VBLANK
    lda #0
    sta VBLANK
    
    ; Color bars section (64 scanlines, 8 bars of 8 scanlines each)
    ldx #8          ; 8 bars
    ldy #0          ; Color index
ColorBarLoop:
    ; Set background color based on bar number
    tya
    asl             ; Multiply by 16 for color hue
    asl
    asl
    asl
    ora #$0E        ; Add brightness
    sta COLUBK
    
    ; Draw 8 scanlines with this color
    ldy #8
ColorBarScanlines:
    sta WSYNC
    dey
    bne ColorBarScanlines
    
    iny             ; Next color
    dex
    bne ColorBarLoop
    
    ; Sprite section (64 scanlines)
    ; Black background
    lda #$00
    sta COLUBK
    
    ; Enable playfield with pattern
    lda #$AA
    sta PF0
    sta PF1
    sta PF2
    
    ldx #64
SpriteLoop:
    sta WSYNC
    dex
    bne SpriteLoop
    
    ; Playfield section (64 scanlines)
    ; Different playfield pattern
    lda #$55
    sta PF0
    lda #$FF
    sta PF1
    lda #$00
    sta PF2
    
    ; Green background
    lda #$C0
    sta COLUBK
    
    ldx #64
PlayfieldLoop:
    sta WSYNC
    dex
    bne PlayfieldLoop
    
    ; Clear playfield
    lda #0
    sta PF0
    sta PF1
    sta PF2
    sta COLUBK
    
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
    
    ; Increment frame counter
    inc FrameCount
    
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
