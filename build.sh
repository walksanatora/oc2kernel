#!/usr/bin/env bash
#build the kernel binary
if [ -v rel ]; then
    cargo build --release
    cp target/riscv64imac-unknown-none-elf/release/oc2kernel .
else
    cargo build
    cp target/riscv64imac-unknown-none-elf/debug/oc2kernel .
fi
llvm-objcopy -O binary oc2kernel kernel.bin
