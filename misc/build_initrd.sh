#!/bin/bash

pushd ../target/initrd/
find . -depth -print | cpio -o > ../initrd.cpio
popd
