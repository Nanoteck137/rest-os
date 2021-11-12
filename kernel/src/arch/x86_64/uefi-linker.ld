ENTRY(kernel_init)

KERNEL_VMA = 0xffffffff80000000;

PHDRS {
	text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
	data PT_LOAD FLAGS(6);
}

SECTIONS {
    . = KERNEL_VMA;

    .text BLOCK(4K) : ALIGN(4K)
    {
        *(.text .text.*)
    } :text

    .rodata BLOCK(4K) : ALIGN(4K)
    {
         *(.rodata .rodata.*)
    } :rodata

    .data BLOCK(4K) : ALIGN(4K)
    {
         *(.data .data.*)
    } :data

    .bss BLOCK(4K) : ALIGN(4K)
    {
        *(.bss .bss.*)
    }

    _end = .;
}
