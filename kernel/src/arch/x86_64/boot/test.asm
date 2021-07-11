section .text

bits 32

kernel_start:
    mov dword [0x1234], 123
    hlt
