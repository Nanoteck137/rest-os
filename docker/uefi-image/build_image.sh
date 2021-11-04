#!/bin/bash

dd if=/dev/zero of=image.img bs=512 count=93750

parted image.img -s -a minimal mklabel gpt
parted image.img -s -a minimal mkpart EFI FAT16 2048s 93716s
parted image.img -s -a minimal toggle 1 boot

dd if=/dev/zero of=part.img bs=512 count=91669
mformat -i part.img -h 32 -t 32 -n 64 -c 1

mmd -i part.img ::/EFI
mmd -i part.img ::/EFI/boot

mcopy -i part.img /data/boot.efi ::/EFI/boot/BOOTX64.efi
mcopy -i part.img /data/startup.nsh ::

dd if=part.img of=image.img bs=512 count=91669 seek=2048 conv=notrunc

cp image.img /data
