ENTRY(kernel_init)

KERNEL_VMA = 0xffffffff80000000;

PHDRS {
	text PT_LOAD FLAGS(5);
	data PT_LOAD FLAGS(6);
}

SECTIONS {
    . = KERNEL_VMA;

    .text :
    {
        *(.text .text.*)
    } :text

    .rodata :
    {
         *(.rodata .rodata.*)
    }

    . = ALIGN(0x1000);

    .data :
    {
         *(.data .data.*)
    } :data

    .bss :
    {
        *(.bss .bss.*)
    }

    . = ALIGN(0x1000);
    _end = .;
}
