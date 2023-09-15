#!/usr/bin/fish
set argv ""
for line in (grep -n "//break-here" src/main.rs | awk '{print $1}')
    set num (echo $line | head -c-2)
    set argv "$argv break main.rs:$num "
end
echo $argv

riscv64-elf-gdb \
    -ex "target remote :3333" \
    -q oc2kernel