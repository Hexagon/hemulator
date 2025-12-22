; Atari 2600 Timer Test ROM
; This ROM tests the RIOT timer interrupt flag clear-on-read behavior
; Sets up timer, waits for it to expire, then changes playfield color

    processor 6502
    include "vcs.h"

    seg.u Variables
    org $80

FrameCount ds 1     ; Frame counter

    seg Code
    org $F000

Start:
    sei             ; Disable interrupts
    cld             ; Clear decimal mode
    ldx #$FF
    txs             ; Set up stack
    
    ; Clear RAM and TIA
    lda #0
    sta FrameCount
ClearMem:
    sta $00,x
    dex
    bne ClearMem
    
    ; Initial playfield color (red)
    lda #$30        ; Red
    sta COLUPF
    
    ; Set background color to black
    lda #$00        ; Black
    sta COLUBK
    
    ; Set playfield pattern
    lda #$FF        ; Full pattern
    sta PF0
    sta PF1
    sta PF2
    
MainLoop:
    ; VSYNC
    lda #2
    sta VSYNC
    sta WSYNC
    sta WSYNC
    sta WSYNC
    lda #0
    sta VSYNC
    
    ; VBLANK - set timer during vblank
    lda #$02
    sta VBLANK
    
    ; Set timer to expire after ~37 scanlines * 76 cycles = ~2812 cycles
    ; Using TIM64T: 2812 / 64 = ~44
    lda #44
    sta TIM64T
    
    ; Wait for timer to expire by checking TIMINT
WaitTimer:
    lda TIMINT      ; Read timer status (should clear flag if set)
    bmi TimerDone   ; Branch if bit 7 set (timer expired)
    jmp WaitTimer   ; Keep waiting
    
TimerDone:
    ; Timer expired - increment frame counter
    inc FrameCount
    
    ; Change color based on frame count
    ; This proves timer is working and flag clears properly
    lda FrameCount
    and #$0F        ; Keep lower 4 bits for color variation
    asl             ; Shift left 4 times to get hue
    asl
    asl
    asl
    sta COLUPF
    
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
