; Atari 2600 Ball Animation Test ROM
; Tests critical TIA rendering features using commercial ROM patterns:
; - Standard horizontal positioning with divide-by-15 + HMOVE fine tuning
; - Animated ball bouncing horizontally across the screen
; - Proper playfield pattern (reflected mode, matching commercial game style)
; - Correct VSYNC/VBLANK/visible/overscan timing (3/37/192/30 scanlines)
;
; Expected behavior:
; - Playfield borders (vertical bars) on left and right edges, reflected
; - Ball moves smoothly left-to-right and right-to-left across screen
; - Ball is 4 pixels wide, using playfield color
; - Background is dark blue, playfield is white, ball is white
;
; This ROM validates:
; 1. HMOVE-based horizontal motion (ball moves 1 pixel per frame)
; 2. Playfield rendering in reflected mode
; 3. Ball rendering and ENABL register behavior
; 4. Standard Atari 2600 frame timing

    processor 6502
    include "vcs.h"

    seg.u Variables
    org $80

BallXPos    ds 1    ; Ball horizontal position (0-159)
BallDir     ds 1    ; Ball direction: 0 = right, 1 = left
FrameCount  ds 1    ; Frame counter for animation
TempDiv     ds 1    ; Temp for positioning divide

    seg Code
    org $F000

Start:
    sei             ; Disable interrupts
    cld             ; Clear decimal mode
    ldx #$FF
    txs             ; Set up stack

    ; Clear all RAM and TIA registers
    lda #0
    ldx #$FF
ClearMem:
    sta $00,x
    dex
    bne ClearMem

    ; Initialize ball position and direction
    lda #80         ; Start at center
    sta BallXPos
    lda #0          ; Moving right
    sta BallDir

    ; Set colors
    lda #$02        ; Dark blue background (hue 0, lum 1)
    sta COLUBK
    lda #$0E        ; White playfield
    sta COLUPF

    ; Set playfield to reflected mode with ball size = 4 pixels
    ; CTRLPF: bit 0 = reflect, bits 4-5 = ball size (10 = 4 pixels)
    lda #%00100001  ; Reflected + 4-pixel ball
    sta CTRLPF

    ; Set playfield pattern (border bars on left side, reflected to right)
    ; PF0 bits 4-7 are used: %11110000 = solid left edge
    lda #$F0
    sta PF0
    ; PF1 bits 7-0 (reversed): %10000000 = one more pixel
    lda #$80
    sta PF1
    ; PF2 bits 0-7: %00000000 = clear middle
    lda #$00
    sta PF2

    ; Enable ball
    lda #$02        ; Bit 1 enables ball
    sta ENABL

;===========================================
; Main Frame Loop
;===========================================
MainLoop:

    ;===================
    ; VSYNC (3 scanlines)
    ;===================
    lda #$02
    sta VSYNC
    sta WSYNC       ; Scanline 1
    sta WSYNC       ; Scanline 2
    sta WSYNC       ; Scanline 3
    lda #0
    sta VSYNC

    ;===================
    ; VBLANK (37 scanlines)
    ;===================
    lda #$02
    sta VBLANK

    ; Set timer for VBLANK period (37 scanlines * 76 cycles = 2812 cycles)
    ; Use TIM64T: 2812 / 64 ≈ 43 (conservative to avoid overrunning)
    lda #43
    sta TIM64T

    ;--- Update ball position ---
    lda BallDir
    bne .MoveLeft

.MoveRight:
    ; Move ball right: need HM value for +1 right = $F0
    inc BallXPos
    lda BallXPos
    cmp #150        ; Right boundary
    bcc .DoneMove
    lda #1          ; Switch to left
    sta BallDir
    jmp .DoneMove

.MoveLeft:
    ; Move ball left: need HM value for +1 left = $10
    dec BallXPos
    lda BallXPos
    cmp #10         ; Left boundary
    bcs .DoneMove
    lda #0          ; Switch to right
    sta BallDir

.DoneMove:
    ;--- Position ball using standard divide-by-15 routine ---
    ; This is the standard Atari 2600 positioning technique:
    ; 1. Divide desired X position by 15 to get coarse position
    ; 2. Use remainder for fine adjustment via HMBL + HMOVE

    lda BallXPos
    ldx #0          ; X will hold quotient (number of 15-clock delays)
    sec
.DivLoop:
    sbc #15
    bcc .DivDone
    inx
    jmp .DivLoop
.DivDone:
    ; A = -(15 - remainder) = remainder - 15 (negative)
    ; The fine offset needs to be converted to an HM value
    ; remainder = A + 15 (0-14)
    ; HM value = (15 - 1 - remainder) shifted to upper nibble, then complemented
    ; Simpler: We store the number of loop iterations in X,
    ; and compute the fine offset from the remainder

    ; A currently holds (remainder - 15), so remainder = A + 15
    adc #15         ; carry is clear from BCC, so this adds 15
    ; A = remainder (0-14)

    ; Convert remainder to HM value:
    ; HM = -((remainder + 1) - 8) = -(remainder - 7) = 7 - remainder
    ; But we need to handle the sign properly for TIA:
    ; Each HM unit is 1 color clock, positive = left, negative = right
    ; We need: fine_offset = remainder (0-14), but the RESP strobe happens
    ; after X*5 + 4 CPU cycles from WSYNC, which is at color clock (X*5+4)*3 = X*15+12
    ; Visible pixel = (X*15 + 12) - 68 + 4 (TIA delay) = X*15 - 52
    ; We want pixel = BallXPos, so fine_adjust = BallXPos - (X*15 - 52)
    ; But it's easier to just use the lookup table approach

    ; Fine position = remainder
    ; HM value to adjust: we need to move (remainder) pixels to the right
    ; from the coarse position. But the coarse position lands at
    ; approximately the right spot already with the delay loop.

    ; Standard approach: store coarse position count in X, fine in A
    ; Use a lookup table for the HM fine offset
    tay             ; Y = remainder (0-14)
    lda FineAdjustTable,y   ; Look up HM value

    ; Now position the ball:
    ; Wait for HBLANK start, run coarse delay, strobe RESBL, set HMBL
    sta WSYNC       ; Wait for start of new scanline
    sta HMCLR       ; Clear all motion registers

.CoarseLoop:
    dex             ; 2 cycles
    bpl .CoarseLoop ; 3 cycles (taken) = 5 cycles = 15 color clocks per iteration

    ; After loop: we're at the right coarse position
    sta RESBL       ; 4 cycles - strobe ball position reset
    sta HMBL        ; Set fine horizontal motion (A already has the value)

    sta WSYNC       ; Wait for next scanline
    sta HMOVE       ; Apply fine motion (must be done during HBLANK)

    ;--- Wait for end of VBLANK ---
.WaitVBLANK:
    lda TIMINT
    bpl .WaitVBLANK ; Wait until timer expires

    sta WSYNC       ; Sync to start of visible area
    lda #0
    sta VBLANK      ; Turn off VBLANK to start visible display

    ;===================
    ; Visible screen (192 scanlines)
    ;===================
    ldx #192
.VisibleLoop:
    sta WSYNC
    dex
    bne .VisibleLoop

    ;===================
    ; Overscan (30 scanlines)
    ;===================
    lda #$02
    sta VBLANK      ; Turn on VBLANK for overscan

    ; Set timer for overscan (30 scanlines * 76 = 2280 cycles)
    ; 2280 / 64 = ~36
    lda #35
    sta TIM64T

    ; Increment frame counter
    inc FrameCount

.WaitOverscan:
    lda TIMINT
    bpl .WaitOverscan

    jmp MainLoop

;===========================================
; Fine Horizontal Position Adjustment Table
; Maps remainder (0-14) to HM register value
; The coarse loop positions at multiples of 15 color clocks
; This table provides the fine adjustment to hit the exact pixel
;===========================================
FineAdjustTable:
    .byte $70   ; remainder 0:  +7 (left 7)
    .byte $60   ; remainder 1:  +6 (left 6)
    .byte $50   ; remainder 2:  +5 (left 5)
    .byte $40   ; remainder 3:  +4 (left 4)
    .byte $30   ; remainder 4:  +3 (left 3)
    .byte $20   ; remainder 5:  +2 (left 2)
    .byte $10   ; remainder 6:  +1 (left 1)
    .byte $00   ; remainder 7:  no adjustment
    .byte $F0   ; remainder 8:  -1 (right 1)
    .byte $E0   ; remainder 9:  -2 (right 2)
    .byte $D0   ; remainder 10: -3 (right 3)
    .byte $C0   ; remainder 11: -4 (right 4)
    .byte $B0   ; remainder 12: -5 (right 5)
    .byte $A0   ; remainder 13: -6 (right 6)
    .byte $90   ; remainder 14: -7 (right 7)

    ; Interrupt vectors
    org $FFFC
    .word Start     ; RESET
    .word Start     ; IRQ/BRK
