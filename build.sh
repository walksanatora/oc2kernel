#!/usr/bin/env bash
#build the kernel binary
cargo build
cp target/riscv64imac-unknown-none-elf/release/oc2kernel .
llvm-objcopy -O binary oc2kernel kernel.bin
