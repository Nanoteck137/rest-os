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

    mov esp, stack_top

    ; Push the ebx register becuase it hold the pointer to the Multiboot
    ; structure needed by the kernel
    push ebx

    ; TODO(patrik): Check multiboot
    ; TODO(patrik): Check if CPUID is available
    ; TODO(patrik): Check if Long mode is available

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
    ; Add the lower p3 table to the p4 table
    mov eax, lower_p3_table
    or eax, 0b11
    mov [p4_table + 0 * 8], eax

    ; Add the lower p2 table to the lower p3 table
    mov eax, lower_p2_table
    or eax, 0b11
    mov [lower_p3_table + 0 * 8], eax

    ; Add the upper p3 table to the p4 table
    mov eax, upper_p3_table
    or eax, 0b11
    mov [p4_table + 511 * 8], eax

    ; Add the upper p2 table to the upper p3 table
    mov eax, upper_p2_table
    or eax, 0b11
    mov [upper_p3_table + 510 * 8], eax

    ; Initialize the counter
    mov ecx, 0

.map_p2_table:
    ; 2-MiB steps per page
    mov eax, 0x200000
    ; Multiply the index
    mul ecx
    ; Add the PRESENT + WRITABLE + HUGE flags to the entry
    or eax, 0b10000011
    ; Add the entry to both the lower and upper p2 tables
    mov [lower_p2_table + ecx * 8], eax
    mov [upper_p2_table + ecx * 8], eax

    ; Increment the counter
    inc ecx
    ; If we the counter reaches 512 then we have mapped all the 512
    ; entries insides the lower and upper p2 tables
    cmp ecx, 512
    ; If the counter is not equal to 512 then continue to map in the entries
    ; needed inside the tables
    jne .map_p2_table

    ; Return from the function
    ret

section .text
bits 64
extern kernel_init
long_mode_start:
    ; NOTE(patrik): Now we are in 64 bit land

    ; Setup all the segment register
    mov ax, 0
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Jump to the upper half of the kernel
    ; To do so we need to add the offset to RIP and we do so by adding
    ; the offset for an jump and then jumping to the location + the offset
    mov rax, .target
    add rax, 0xffffffff80000000
    jmp rax
.target:

    ; Pop of the Multiboot Structure pointer we pushed on previously
    pop rdi

    ; Set rax to the kernel_init function from rust becuase i did have a
    ; problem with just 'call kernel_init' and this fixed that problem
    mov rax, kernel_init
    ; Call the kernel_init function
    call rax

    hlt

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
    resb 4096
stack_top:

section .rodata
gdt64:
    dq 0 ; zero entry
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
.pointer:
    dw $ - gdt64 - 1
    dq gdt64
