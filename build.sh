#!/usr/bin/env bash

cargo build --profile lto
elf2uf2-rs target/thumbv6m-none-eabi/lto/value-counter
sudo mkdir /mnt/usb1 2>/dev/null || true
sudo mount /dev/sda1 /mnt/usb1
sudo cp target/thumbv6m-none-eabi/lto/value-counter.uf2 /mnt/usb1
sudo umount /mnt/usb1
