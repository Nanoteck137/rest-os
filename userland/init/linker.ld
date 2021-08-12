ENTRY(_start)

SECTIONS {
    . = 1M;

    .text :
    {
        *(.text .text.*)
    }

    .data :
    {
        *(.data .data.*)
    }

    .rodata :
    {
        *(.rodata .rodata.*)
    }

    .bss :
    {
        *(.bss .bss.*)
    }
}
