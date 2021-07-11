section .multiboot_header
header_start:
    dd 0xe85250d6                ; magic number (multiboot 2)
    dd 0                         ; architecture 0 (protected mode i386)
    dd header_end - header_start ; header length
    ; checksum
    dd 0x100000000 - (0xe85250d6 + 0 + (header_end - header_start))

    ; insert optional multiboot tags here

    ; required end tag
    dw 0    ; type
    dw 0    ; flags
    dd 8    ; size
header_end:

section .text

global boot_entry
bits 32

boot_entry:
    cli
    cld

    mov esp, stack_bottom
    ; TODO(patrik): Setup an temporary stack


    ; TODO(patrik): Check multiboot
    ; TODO(patrik): Check if CPUID is available
    ; TODO(patrik): Check if Long mode is available

    ; The plan
    ; Identity map the first 512 MiB of memory
    ; Enter Long Mode
    ; Then map in the kernel at the upper memory region, becuase we need to
    ; use 64 bit instruction to change the page table correctly

    call setup_page_tables
    call enable_paging

    lgdt [gdt64.pointer]

    mov dword [0xb8000], 0x2f4b2f4f

    jmp 0x08:long_mode_start

enable_paging:
    ; Setup the correct page table to use
    mov eax, p4_table
    mov cr3, eax

    ; Enable the PAE (Physical Address Extention) flag inside
    ; the ´cr4´ register
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; Setup Long mode by setting a bit inside the EFER MSR
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ; Enable paging by setting a bit inside the ´cr0´ register
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    ret

setup_page_tables:
    mov eax, p3_table
    or eax, 0b11
    mov [p4_table + 0 * 8], eax

    mov eax, p2_table
    or eax, 0b11
    mov [p3_table + 0 * 8], eax

    mov ecx, 0

.map_p2_table:
    add eax, 0x200000
    mul ecx
    or eax, 0b10000011
    mov [p2_table + ecx * 8], eax

    inc ecx
    cmp ecx, 256
    jne .map_p2_table

    ret

bits 64
long_mode_start:
    mov rax, 0x2f592f412f4b2f4f
    mov qword [0xb8000], rax
    hlt

section .bss
align 4096
p4_table:
    resb 4096
p3_table:
    resb 4096
p2_table:
    resb 4096
stack_bottom:
    resb 1024
stack_top:

section .rodata
gdt64:
    dq 0 ; zero entry
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
.pointer:
    dw $ - gdt64 - 1
    dq gdt64
