ENTRY(kernel_init)

KERNEL_VMA = 0xffffffff80000000;

PHDRS {
	text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
	data PT_LOAD FLAGS(6);
    heap PT_LOAD FLAGS(6);
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

    .heap BLOCK(4K) : ALIGN(4K)
    {
        _heap_start = .;
        . = . + (1 * 1024 * 1024);
        _heap_end = .;
    } :heap

    _end = .;
}
