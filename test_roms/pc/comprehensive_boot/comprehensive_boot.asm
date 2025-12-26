; Comprehensive PC boot test ROM
; This bootloader performs extensive CPU, memory, and disk I/O testing
; It attempts to replicate the DOS boot process to help diagnose FreeDOS/MS-DOS freeze issues
; Assembled with NASM: nasm -f bin comprehensive_boot.asm -o comprehensive_boot.bin

BITS 16                 ; 16-bit real mode
CPU 8086                ; Target 8086 CPU (no 386+ instructions)
ORG 0x7C00              ; Boot sector loads here

start:
    ; Setup segments
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0xFFFE
    sti

    ; Print banner
    mov si, msg_banner
    call print_string

    ; ===== CPU TESTS =====
    mov si, msg_cpu
    call print_string

    ; Basic arithmetic
    mov ax, 0x1234
    mov bx, 0x5678
    add ax, bx
    cmp ax, 0x68AC
    je .cpu1
    jmp fail
.cpu1:
    sub ax, bx
    cmp ax, 0x1234
    je .cpu2
    jmp fail

.cpu2:
    ; Logical operations
    mov ax, 0xFF00
    mov bx, 0x00FF
    and ax, bx
    jz .cpu3
    jmp fail
.cpu3:
    mov ax, 0xFF00
    or ax, bx
    cmp ax, 0xFFFF
    je .cpu4
    jmp fail

.cpu4:
    ; Shift operations
    mov ax, 1
    shl ax, 1
    shl ax, 1
    cmp ax, 4
    je .cpu5
    jmp fail
.cpu5:
    shr ax, 1
    cmp ax, 2
    je .cpu_ok
    jmp fail

.cpu_ok:
    mov si, msg_ok
    call print_string

    ; ===== MEMORY TESTS =====
    mov si, msg_mem
    call print_string

    ; Test read/write at different addresses
    mov di, 0x0500
    mov ax, 0xAA55
    mov [di], ax
    cmp [di], ax
    je .mem1
    jmp fail

.mem1:
    ; Pattern test
    mov di, 0x2000
    mov cx, 64
    mov ax, 0x5AA5
.mfill:
    mov [di], ax
    add di, 2
    loop .mfill
    
    mov di, 0x2000
    mov cx, 64
.mverify:
    cmp [di], ax
    jne .mem_fail
    add di, 2
    loop .mverify
    jmp .mem_ok

.mem_fail:
    jmp fail

.mem_ok:
    mov si, msg_ok
    call print_string

    ; ===== DISK I/O TESTS =====
    mov si, msg_disk
    call print_string

    ; Reset disk
    xor ax, ax
    xor dl, dl
    int 0x13
    jc .disk_fail

    ; Read sector 2
    mov ah, 0x02
    mov al, 1
    xor ch, ch
    mov cl, 2
    xor dh, dh
    xor dl, dl
    mov bx, 0x0500
    int 0x13
    jc .disk_fail
    cmp al, 1
    jne .disk_fail

    ; Read sector 3
    mov cl, 3
    mov bx, 0x0700
    int 0x13
    jc .disk_fail

    ; Read from head 1 (multi-track)
    mov cl, 1
    mov dh, 1
    mov bx, 0x0900
    int 0x13
    jc .disk_fail
    jmp .disk_ok

.disk_fail:
    jmp fail

.disk_ok:
    mov si, msg_ok
    call print_string

    ; ===== PROGRAM LOADING TEST =====
    mov si, msg_load
    call print_string

    ; Read 5 consecutive sectors (simulating DOS file load)
    mov cx, 5
    mov cl, 5           ; Start at sector 5
    xor dh, dh
    mov bx, 0x1000

.lloop:
    push cx
    mov ah, 0x02
    mov al, 1
    xor ch, ch
    xor dl, dl
    int 0x13
    jc .load_fail
    add bx, 0x0200
    inc cl
    pop cx
    loop .lloop
    jmp .load_ok

.load_fail:
    pop cx
    jmp fail

.load_ok:
    mov si, msg_ok
    call print_string

    ; ===== ALL TESTS PASSED =====
    mov si, msg_pass
    call print_string
    mov si, msg_prompt
    call print_string

    ; Simple prompt loop
.ploop:
    xor ah, ah
    int 0x16
    cmp al, 'Q'
    je .halt
    cmp al, 'q'
    je .halt
    mov ah, 0x0E
    int 0x10
    cmp al, 0x0D
    jne .ploop
    mov al, 0x0A
    int 0x10
    mov si, msg_prompt
    call print_string
    jmp .ploop

.halt:
    mov si, msg_halt
    call print_string
    cli
    hlt

fail:
    mov si, msg_fail
    call print_string
    cli
    hlt

; ===== HELPER FUNCTIONS =====

; Print null-terminated string pointed to by SI
print_string:
    push ax
    push si
.loop:
    lodsb               ; Load byte from [SI] into AL, increment SI
    test al, al         ; Check if AL is 0 (null terminator)
    jz .done
    mov ah, 0x0E        ; INT 10h, AH=0Eh: Teletype output
    int 0x10
    jmp .loop
.done:
    pop si
    pop ax
    ret

; ===== STRINGS =====
msg_banner:     db 0x0D, 0x0A, '=== PC Boot Test ===', 0x0D, 0x0A, 0
msg_cpu:        db 'CPU... ', 0
msg_mem:        db 'MEM... ', 0
msg_disk:       db 'DISK... ', 0
msg_load:       db 'LOAD... ', 0
msg_ok:         db 'OK', 0x0D, 0x0A, 0
msg_fail:       db 'FAIL', 0x0D, 0x0A, 0
msg_pass:       db 0x0D, 0x0A, 'All OK!', 0x0D, 0x0A, 0
msg_prompt:     db 'BOOT> ', 0
msg_halt:       db 0x0D, 0x0A, 'Halted.', 0x0D, 0x0A, 0

; Pad to 510 bytes
times 510-($-$$) db 0

; Boot signature
dw 0xAA55
