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
    mov eax, lower_p3_table
    or eax, 0b11
    mov [p4_table + 0 * 8], eax

    mov eax, lower_p2_table
    or eax, 0b11
    mov [lower_p3_table + 0 * 8], eax

    mov ecx, 0

.map_p2_table:
    mov eax, 0x200000
    mul ecx
    or eax, 0b10000011
    mov [lower_p2_table + ecx * 8], eax

    inc ecx
    cmp ecx, 256
    jne .map_p2_table

    ret

section .text
bits 64
extern kernel_init
long_mode_start:
    ; NOTE(patrik): Now we are in 64 bit land

    call setup_upper_half_paging
    ; Reload the cr3 to flush the caches (I don't think this need to happen
    ;   but to be on the safe side)
    mov rax, cr3
    mov cr3, rax

    ; Jump to the upper half of the kernel
    ; To do so we need to add the offset to RIP and we do so by adding
    ; the offset for an jump and then jumping to the location + the offset
    mov rax, .target
    add rax, 0xffffffff80000000
    jmp rax
.target:

    mov rax, 0x2f592f412f4b2f4f
    mov qword [0xb8000], rax

    mov rax, kernel_init
    call rax

    hlt

setup_upper_half_paging:
    mov rax, upper_p3_table
    or rax, 0b11
    mov [p4_table + 511 * 8], rax

    mov rax, upper_p2_table
    or rax, 0b11
    mov [upper_p3_table + 510 * 8], rax

    mov rcx, 0
.map_p2_table:
    mov rax, 0x200000
    mul rcx
    or rax, 0b10000011
    mov [upper_p2_table + rcx * 8], rax

    inc rcx
    cmp rcx, 256
    jne .map_p2_table

    ret

section .bss
align 4096
p4_table:
    resb 4096

; NOTE(patrik): The upper tables are used for the mapping for
; the Higher half kernel
upper_p3_table:
    resb 4096
upper_p2_table:
    resb 4096

; NOTE(patrik): The lower tables are used for the mapping for
; the Lower half kernel
lower_p3_table:
    resb 4096
lower_p2_table:
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
