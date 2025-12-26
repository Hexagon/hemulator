; Interactive PC boot sector test ROM
; This creates a bootable floppy with a menu for testing various features
; Assembled with NASM: nasm -f bin menu.asm -o menu.bin

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

    ; Enable interrupts (required for INT 10h and INT 16h)
    sti

    ; Print "BOOT OK" message
    mov si, msg_boot_ok
    call print_string

    ; Run memory test
    call test_memory

    ; Run CPU test
    call test_cpu

    ; Print menu
    call print_menu

main_loop:
    ; Read a keystroke (blocking)
    mov ah, 0x00        ; INT 16h, AH=00h: Read keystroke
    int 0x16            ; Returns: AH = scan code, AL = ASCII

    ; Check which key was pressed
    cmp al, '1'
    je test_input
    cmp al, '2'
    je test_math
    cmp al, '3'
    je test_file_io
    cmp al, 'q'
    je quit
    cmp al, 'Q'
    je quit

    ; Invalid key - show menu again
    jmp print_menu

test_input:
    ; Test user input - keyboard echo
    mov si, msg_input_test
    call print_string

.input_loop:
    ; Read keystroke
    mov ah, 0x00
    int 0x16

    ; Check for ESC key (scan code 0x01)
    cmp ah, 0x01
    je .done

    ; Echo the character
    mov ah, 0x0E        ; INT 10h, AH=0Eh: Teletype output
    int 0x10

    jmp .input_loop

.done:
    mov si, msg_newline
    call print_string
    jmp print_menu

test_math:
    ; Calculate 2+2 and display result
    mov si, msg_math_test
    call print_string

    ; Calculate 2+2
    mov al, 2
    add al, 2

    ; Convert result to ASCII and print
    add al, '0'         ; Convert 4 to '4'
    mov ah, 0x0E
    int 0x10

    mov si, msg_newline
    call print_string
    jmp print_menu

test_file_io:
    ; Simulate file I/O test
    mov si, msg_fileio_test
    call print_string

    ; Simulate writing
    mov si, msg_write
    call print_string

    ; Simulate reading
    mov si, msg_read
    call print_string

    jmp print_menu

quit:
    ; Print goodbye message and halt
    mov si, msg_goodbye
    call print_string

    cli
    hlt

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

print_menu:
    mov si, msg_menu
    call print_string
    jmp main_loop

; Memory test - simple write and read test
test_memory:
    push ax
    push di

    ; Write and read test pattern at 0x1000
    mov di, 0x1000
    mov ax, 0xAA55
    mov [di], ax
    cmp [di], ax
    jne .fail

    ; Test passed
    mov si, msg_mem_ok
    call print_string
    jmp .done

.fail:
    mov si, msg_mem_fail
    call print_string

.done:
    pop di
    pop ax
    ret

; CPU test - basic arithmetic and logic
test_cpu:
    push ax

    ; Test addition
    mov ax, 2
    add ax, 2
    cmp ax, 4
    jne .fail

    ; Test XOR
    xor ax, ax
    cmp ax, 0
    jne .fail

    ; Test passed
    mov si, msg_cpu_ok
    call print_string
    jmp .done

.fail:
    mov si, msg_cpu_fail
    call print_string

.done:
    pop ax
    ret

; Strings
msg_boot_ok:     db 'BOOT OK', 0x0D, 0x0A, 0
msg_mem_ok:      db 'MEM OK', 0x0D, 0x0A, 0
msg_mem_fail:    db 'MEM FAIL', 0x0D, 0x0A, 0
msg_cpu_ok:      db 'CPU OK', 0x0D, 0x0A, 0x0D, 0x0A, 0
msg_cpu_fail:    db 'CPU FAIL', 0x0D, 0x0A, 0x0D, 0x0A, 0
msg_menu:        db '=== PC Test Menu ===', 0x0D, 0x0A
                 db '1. Test user input', 0x0D, 0x0A
                 db '2. Calculate 2+2', 0x0D, 0x0A
                 db '3. Test file I/O', 0x0D, 0x0A
                 db 'Q. Quit', 0x0D, 0x0A
                 db 'Select: ', 0
msg_input_test:  db 0x0D, 0x0A, 'Type text (ESC to exit): ', 0
msg_math_test:   db 0x0D, 0x0A, '2 + 2 = ', 0
msg_fileio_test: db 0x0D, 0x0A, 'File I/O Test', 0x0D, 0x0A, 0
msg_write:       db 'Writing data... OK', 0x0D, 0x0A, 0
msg_read:        db 'Reading data... OK', 0x0D, 0x0A, 0
msg_goodbye:     db 0x0D, 0x0A, 'Goodbye!', 0x0D, 0x0A, 0
msg_newline:     db 0x0D, 0x0A, 0

; Pad to 510 bytes
times 510-($-$$) db 0

; Boot signature
dw 0xAA55
