; File I/O test bootloader
; This bootloader attempts to read a file from disk using INT 21h
; It demonstrates the file I/O operations needed for DOS bootloaders
; Assembled with NASM: nasm -f bin fileio_test.asm -o fileio_test.bin

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

    ; Enable interrupts
    sti

    ; Print banner
    mov si, msg_banner
    call print_string

    ; Test 1: Try to open a file (IO.SYS)
    mov si, msg_test1
    call print_string

    ; INT 21h, AH=3Dh: Open file
    ; AL = access mode (0 = read only)
    ; DS:DX = pointer to ASCIIZ filename
    mov ah, 0x3D        ; Open file
    mov al, 0x00        ; Read only
    mov dx, filename_iosys
    int 0x21            ; Call DOS API
    jc .open_failed     ; If CF set, error

    ; File opened successfully - AX = file handle
    mov [file_handle], ax
    mov si, msg_success
    call print_string
    jmp .read_file

.open_failed:
    ; AX = error code
    mov si, msg_open_fail
    call print_string
    ; Print error code in AX
    call print_ax_hex
    jmp .test2

.read_file:
    ; Test 2: Try to read from the file
    mov si, msg_test2
    call print_string

    ; INT 21h, AH=3Fh: Read from file
    ; BX = file handle
    ; CX = number of bytes to read
    ; DS:DX = pointer to buffer
    mov ah, 0x3F        ; Read file
    mov bx, [file_handle]
    mov cx, 64          ; Read 64 bytes
    mov dx, read_buffer
    int 0x21
    jc .read_failed

    ; AX = number of bytes actually read
    mov si, msg_success
    call print_string
    ; Print number of bytes read
    call print_ax_hex

    ; Close the file
    mov ah, 0x3E        ; Close file
    mov bx, [file_handle]
    int 0x21

    jmp .test2

.read_failed:
    mov si, msg_read_fail
    call print_string
    call print_ax_hex

.test2:
    ; Test 3: Try to write a file
    mov si, msg_test3
    call print_string

    ; INT 21h, AH=3Ch: Create file
    ; CX = file attributes (0 = normal)
    ; DS:DX = pointer to ASCIIZ filename
    mov ah, 0x3C        ; Create file
    mov cx, 0x00        ; Normal attributes
    mov dx, filename_test
    int 0x21
    jc .create_failed

    ; File created - AX = file handle
    mov [file_handle], ax
    mov si, msg_success
    call print_string

    ; Write some data
    mov ah, 0x40        ; Write to file
    mov bx, [file_handle]
    mov cx, test_data_len
    mov dx, test_data
    int 0x21
    jc .write_failed

    mov si, msg_success
    call print_string

    ; Close the file
    mov ah, 0x3E        ; Close file
    mov bx, [file_handle]
    int 0x21

    jmp .done

.create_failed:
    mov si, msg_create_fail
    call print_string
    call print_ax_hex
    jmp .done

.write_failed:
    mov si, msg_write_fail
    call print_string
    call print_ax_hex

.done:
    mov si, msg_done
    call print_string

    ; Halt
hang:
    hlt
    jmp hang

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

; Print AX register as hexadecimal
print_ax_hex:
    push ax
    push bx
    push cx
    push dx

    ; Print "0x"
    mov al, '0'
    mov ah, 0x0E
    int 0x10
    mov al, 'x'
    int 0x10

    ; Print high byte
    mov dx, ax
    mov cl, 4
    shr dx, cl
    shr dx, cl
    shr dx, cl
    shr dx, cl
    and dx, 0x0F
    add dl, '0'
    cmp dl, '9'
    jle .print1
    add dl, 7
.print1:
    mov al, dl
    mov ah, 0x0E
    int 0x10

    ; Print remaining nibbles (simplified)
    mov si, msg_newline
    call print_string

    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Data
msg_banner:      db 0x0D, 0x0A, '=== File I/O Test ===', 0x0D, 0x0A, 0
msg_test1:       db 'Test 1: Opening IO.SYS... ', 0
msg_test2:       db 0x0D, 0x0A, 'Test 2: Reading file... ', 0
msg_test3:       db 0x0D, 0x0A, 'Test 3: Creating TEST.TXT... ', 0
msg_success:     db 'OK ', 0
msg_open_fail:   db 'FAILED (Open) Code: ', 0
msg_read_fail:   db 'FAILED (Read) Code: ', 0
msg_create_fail: db 'FAILED (Create) Code: ', 0
msg_write_fail:  db 'FAILED (Write) Code: ', 0
msg_done:        db 0x0D, 0x0A, 'Tests complete.', 0x0D, 0x0A, 0
msg_newline:     db 0x0D, 0x0A, 0

filename_iosys:  db 'IO.SYS', 0
filename_msdos:  db 'MSDOS.SYS', 0
filename_test:   db 'TEST.TXT', 0

test_data:       db 'Hello from file I/O test!', 0x0D, 0x0A
test_data_len:   equ $ - test_data

; Variables
file_handle:     dw 0
read_buffer:     times 64 db 0

; Pad to 510 bytes
times 510-($-$$) db 0

; Boot signature
dw 0xAA55
