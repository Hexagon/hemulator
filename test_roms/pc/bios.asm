; Custom BIOS for IBM PC/XT Emulator
; Provides basic interrupt handlers and disk I/O support
; Assembled with NASM: nasm -f bin -o bios.bin bios.asm

[BITS 16]
[ORG 0x0000]

; Main boot code starts at beginning
boot_main:
    cli                         
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0xFFFE             
    
    ; Set up INT 0x13 (Disk Services) 
    mov word [0x4C], int_13h
    mov word [0x4E], 0xF000
    
    sti                         
    
    ; Halt
    jmp halt_system

; INT 0x13 - Disk Services
int_13h:
    clc                         
    xor ah, ah                 
    iret

; Halt system
halt_system:
    cli                         
    hlt                         
    jmp halt_system            

; Pad to 0xFFF0 (where entry point goes)
times 0xFFF0-($-$$) db 0

; Entry point at 0xFFF0 - CPU reset vector
entry_point:
    jmp 0xF000:boot_main       ; 5 bytes: EA 00 00 00 F0

; Pad to 0xFFF5
times 0xFFF5-($-$$) db 0

; BIOS date
db "12/22/24"                  ; 8 bytes

; System model byte
db 0xFE

; Two final bytes to reach exactly 64KB
db 0x00
db 0x00
