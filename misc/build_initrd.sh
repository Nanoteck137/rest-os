#!/bin/bash

pushd ../target/initrd/
find . -depth -print | cpio -H newc -o > ../initrd.cpio
popd
