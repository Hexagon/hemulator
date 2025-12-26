; Simple PC boot sector test ROM
; This creates a minimal bootable floppy that writes "BOOT OK" to video memory
; Assembled with NASM: nasm -f bin boot.asm -o boot.bin

BITS 16                 ; 16-bit real mode
ORG 0x7C00              ; Boot sector loads here

start:
    ; Clear interrupts
    cli

    ; Setup segments
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0xFFFE

    ; Write "BOOT OK" to text mode video memory at 0xB8000
    mov ax, 0xB800
    mov es, ax
    xor di, di

    ; Write 'B' with green attribute (0x02)
    mov byte [es:di], 'B'
    mov byte [es:di+1], 0x02
    add di, 2

    ; Write 'O'
    mov byte [es:di], 'O'
    mov byte [es:di+1], 0x02
    add di, 2

    ; Write 'O'
    mov byte [es:di], 'O'
    mov byte [es:di+1], 0x02
    add di, 2

    ; Write 'T'
    mov byte [es:di], 'T'
    mov byte [es:di+1], 0x02
    add di, 2

    ; Write ' '
    mov byte [es:di], ' '
    mov byte [es:di+1], 0x02
    add di, 2

    ; Write 'O'
    mov byte [es:di], 'O'
    mov byte [es:di+1], 0x02
    add di, 2

    ; Write 'K'
    mov byte [es:di], 'K'
    mov byte [es:di+1], 0x02

    ; Enable interrupts
    sti

    ; Halt (infinite loop)
hang:
    hlt
    jmp hang

; Pad to 510 bytes
times 510-($-$$) db 0

; Boot signature
dw 0xAA55
