ENTRY(boot_entry)

KERNEL_VMA = 0xfff8000000000000;

SECTIONS {
    . = 1M;

    .bootstrap :
    {
        /* ensure that the multiboot header is at the beginning */
        target/boot.o *(.multiboot_header)
        target/boot.o (.text)
    }

    . += KERNEL_VMA;

    .text : AT(ADDR(.text) - KERNEL_VMA)
    {
        *(EXCLUDE_FILE(*target/boot.o) .text)
    }
}
