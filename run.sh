``qemu-system-riscv64 -kernel kernel.bin \
    -machine virt \
    -cpu rv64 \
    -serial stdio \
    -device virtio-blk-device,serial=deadbeef,drive=disk0 \
    -global virtio-mmio.force-legacy=false \
    -drive id=disk0,format=raw,if=none,file=disk0.img \
    -trace virtio_mmio*``