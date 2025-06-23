#!/usr/bin/env bash

BSP=rpi5 make kernel8.img
cp kernel8.img jtag_boot_rpi5.img
rm kernel8.img
