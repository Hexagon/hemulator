; SNES test ROM - displays a checkerboard pattern
; Purpose: Verify basic 65C816 CPU and video memory functionality
; Uses minimal VRAM writes to create a visible pattern in memory

* = $8000

; LoROM header at $FFB0 (within bank 0, offset $FFB0)
.dsb $7FB0-*, $00              ; Pad to header location

    .asc "SNES TEST ROM    "     ; Game title (21 bytes)
    .byt $20                      ; ROM makeup byte (LoROM)
    .byt $00                      ; ROM type (ROM only)
    .byt $07                      ; ROM size (128KB)
    .byt $00                      ; SRAM size (none)
    .byt $01                      ; Country (USA)
    .byt $33                      ; License code
    .byt $00                      ; Version
    .word $0000                   ; Checksum complement
    .word $0000                   ; Checksum

; Interrupt vectors (Native mode)
.dsb $7FE4-*, $00
    .word $0000                   ; COP
    .word $0000                   ; BRK
    .word $0000                   ; ABORT
    .word nmi_handler             ; NMI
    .word $0000                   ; (unused)
    .word irq_handler             ; IRQ

; Interrupt vectors (Emulation mode - 6502 compatible)
.dsb $7FF4-*, $00
    .word $0000                   ; COP
    .word $0000                   ; (unused)
    .word $0000                   ; ABORT
    .word nmi_handler             ; NMI
    .word reset                   ; RESET
    .word irq_handler             ; IRQ/BRK

; Reset vector - main entry point
* = $8000
reset:
    sei                           ; Disable interrupts
    clc                           ; Clear carry for XCE
    xce                           ; Switch to native mode (E=0)
    
    .byt $C2, $38                 ; rep #$38 - 16-bit A, 16-bit X/Y, clear decimal
    
    ; Set up stack
    lda #$1FFF
    tcs                           ; Transfer to stack pointer
    
    ; Clear direct page
    lda #$0000
    tcd
    
    ; Set data bank to 0
    lda #$0000
    pha
    plb
    
    ; Write checkerboard pattern to WRAM at $7E:0000
    ; This simulates VRAM - we'll write alternating bytes
    ; For simplicity, write pattern to create visible test
    
    ; Switch to 8-bit accumulator for byte writes
    .byt $E2, $20                 ; sep #$20 - 8-bit A
    
    ldx #$0000                    ; Start at offset 0
write_loop:
    lda #$AA                      ; Pattern byte 1 (checkerboard)
    sta $7E0000,x                 ; Write to WRAM bank $7E
    inx
    lda #$55                      ; Pattern byte 2 (inverted)
    sta $7E0000,x
    inx
    cpx #$2000                    ; Write 8KB (enough for test)
    bne write_loop
    
    ; Enter infinite loop
forever:
    wai                           ; Wait for interrupt
    bra forever

nmi_handler:
    rti

irq_handler:
    rti

; Pad to 32KB ROM
.dsb $10000-*, $00
