ENTRY(boot_entry)

KERNEL_VMA = 0xffffffff80000000;

SECTIONS {
    . = 1M;

    .bootstrap :
    {
        /* ensure that the multiboot header is at the beginning */
        target/boot.o (.multiboot_header)
        target/boot.o (.text)
        target/boot.o (.bss)
        target/boot.o (.rodata)
    }

    . += KERNEL_VMA;

    .text : AT(ADDR(.text) - KERNEL_VMA)
    {
        *(EXCLUDE_FILE(*target/boot.o) .text .text.*)
    }

    .data : AT(ADDR(.data) - KERNEL_VMA)
    {
        *(EXCLUDE_FILE(*target/boot.o) .data .data.*)
    }

    .rodata : AT(ADDR(.rodata) - KERNEL_VMA)
    {
        *(EXCLUDE_FILE(*target/boot.o) .rodata .rodata.*)
    }

    .bss : AT(ADDR(.bss) - KERNEL_VMA)
    {
        *(EXCLUDE_FILE(*target/boot.o) .bss .bss.*)

    }

    . = ALIGN(4096);
    _end = .;
}
