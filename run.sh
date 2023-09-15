qemu-system-riscv64 -kernel kernel.bin \
    -machine virt \
    -cpu rv64 \
    -nographic \
    -global virtio-mmio.force-legacy=false \
    \
    -device virtio-blk-device,serial=deadbeef,drive=disk0 \
    -drive id=disk0,format=raw,if=none,file=disk0.img \
    \
    -device virtio-serial \
    -chardev socket,path=./periph.sock,server,nowait,id=peripheral \
    -device virtconsole,name=jobsfoo,chardev=peripheral,name=net.walksanator.peripheral_sock \
    \
    -gdb tcp::3333 \
    -S
    #-device virtio-serial \
    #-chardev socket,path=./periphs-f6301f70-3dfd-4c06-a137-145e45d7c558,server,nowait,id=peripherals-f6301f70-3dfd-4c06-a137-145e45d7c558 \
    #-device virtconsole,name=peripherals-hlapi,chardev=peripherals-f6301f70-3dfd-4c06-a137-145e45d7c558,name=net.walksanator.mcqemu-peripheral
    #-S \
    #-trace virtio_mmio*
