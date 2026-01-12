; SNES breakpoint test ROM
; Purpose: Test breakpoint and instruction tracing functionality
; This ROM performs a simple loop that can be used to test breakpoint hits

.p816                       ; 65816 processor
.a8                         ; 8-bit accumulator by default
.i8                         ; 8-bit index registers by default

.segment "HEADER"
    ; SNES ROM header (internal header at $FFB0-$FFDF for LoROM)
    ; Title (21 bytes)
    .byte "BREAKPOINT TEST      "
    
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
    
    ; Initialize test counter in WRAM
    lda #$00
    sta $7E0000             ; Counter low byte
    sta $7E0001             ; Counter high byte
    
    ; Write initial value to INIDISP (force blank)
    lda #$80
    sta $2100
    
    ; Main loop - increment counter and write to registers
MAIN_LOOP:
    ; Increment 16-bit counter at $7E0000
    rep #$20                ; 16-bit accumulator
    .a16
    lda $7E0000
    inc a
    sta $7E0000
    sep #$20                ; Back to 8-bit
    .a8
    
    ; Write counter low byte to $2100 (INIDISP)
    lda $7E0000
    sta $2100
    
    ; Write counter high byte to $4200 (NMITIMEN)
    lda $7E0001
    sta $4200
    
    ; Breakpoint target: Set this address as breakpoint
    ; PC will be at $8050 (in bank 0) after 100 iterations
BREAKPOINT_TARGET:
    nop
    nop
    nop
    
    ; Check if counter reached 100
    rep #$20                ; 16-bit accumulator
    .a16
    lda $7E0000
    cmp #100
    sep #$20                ; Back to 8-bit
    .a8
    bne MAIN_LOOP
    
    ; After 100 iterations, enable NMI and unblank screen
    lda #$81                ; Enable NMI (bit 7)
    sta $4200
    
    lda #$0F                ; Unblank screen, max brightness
    sta $2100
    
    ; Final infinite loop
FINAL_LOOP:
    wai                     ; Wait for interrupt
    jmp FINAL_LOOP

NMI:
    rti

IRQ:
    rti
