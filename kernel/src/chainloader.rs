// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! Minimal EL2, MMU-off UART chainloader.

const PAYLOAD_LOAD_ADDR: usize = 0x8_0000;

#[cfg(feature = "bsp_rpi3")]
const GPIO_BASE: usize = 0x3f20_0000;
#[cfg(feature = "bsp_rpi3")]
const UART_BASE: usize = 0x3f20_1000;

#[cfg(feature = "bsp_rpi4")]
const GPIO_BASE: usize = 0xfe20_0000;
#[cfg(feature = "bsp_rpi4")]
const UART_BASE: usize = 0xfe20_1000;

#[cfg(all(feature = "bsp_rpi5", not(feature = "early-uart")))]
const GPIO_BASE: usize = 0x1f_000d_0000;
#[cfg(all(feature = "bsp_rpi5", not(feature = "early-uart")))]
const UART_BASE: usize = 0x1c_0003_0000;

#[cfg(all(feature = "bsp_rpi5", feature = "early-uart"))]
const UART_BASE: usize = 0x10_7d00_1000;

const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
#[cfg(not(all(feature = "bsp_rpi5", feature = "early-uart")))]
const UART_IBRD: usize = 0x24;
#[cfg(not(all(feature = "bsp_rpi5", feature = "early-uart")))]
const UART_FBRD: usize = 0x28;
#[cfg(not(all(feature = "bsp_rpi5", feature = "early-uart")))]
const UART_LCR_H: usize = 0x2c;
#[cfg(not(all(feature = "bsp_rpi5", feature = "early-uart")))]
const UART_CR: usize = 0x30;
#[cfg(not(all(feature = "bsp_rpi5", feature = "early-uart")))]
const UART_ICR: usize = 0x44;

const FR_BUSY: u32 = 1 << 3;
const FR_RXFE: u32 = 1 << 4;
const FR_TXFF: u32 = 1 << 5;

fn read32(address: usize) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

fn write32(address: usize, value: u32) {
    unsafe { core::ptr::write_volatile(address as *mut u32, value) }
}

#[cfg(feature = "bsp_rpi3")]
fn short_delay() {
    for _ in 0..150 {
        core::hint::spin_loop();
    }
}

#[cfg(not(all(feature = "bsp_rpi5", feature = "early-uart")))]
fn init_gpio() {
    let gpfsel1 = GPIO_BASE + 0x04;
    let mut gpfsel1_value = read32(gpfsel1);
    gpfsel1_value &= !((0b111 << 12) | (0b111 << 15));
    gpfsel1_value |= (0b100 << 12) | (0b100 << 15);
    write32(gpfsel1, gpfsel1_value);

    #[cfg(feature = "bsp_rpi3")]
    {
        write32(GPIO_BASE + 0x94, 0);
        short_delay();
        write32(GPIO_BASE + 0x98, (1 << 14) | (1 << 15));
        short_delay();
        write32(GPIO_BASE + 0x94, 0);
        write32(GPIO_BASE + 0x98, 0);
    }

    #[cfg(any(feature = "bsp_rpi4", feature = "bsp_rpi5"))]
    {
        let pulls = GPIO_BASE + 0xe4;
        let pull_config = read32(pulls) & !((0b11 << 28) | (0b11 << 30));
        write32(pulls, pull_config);
    }
}

#[cfg(not(all(feature = "bsp_rpi5", feature = "early-uart")))]
fn init_uart() {
    init_gpio();
    flush();
    write32(UART_BASE + UART_CR, 0);
    write32(UART_BASE + UART_ICR, 0x7ff);

    // The configured 48 MHz UART clock yields 115200 baud with divisors 26 and 3.
    write32(UART_BASE + UART_IBRD, 26);
    write32(UART_BASE + UART_FBRD, 3);
    write32(UART_BASE + UART_LCR_H, (0b11 << 5) | (1 << 4));
    write32(UART_BASE + UART_CR, (1 << 0) | (1 << 8) | (1 << 9));
}

#[cfg(all(feature = "bsp_rpi5", feature = "early-uart"))]
fn init_uart() {
    // The firmware configures the RPi 5 debug UART before loading the kernel.
    flush();
}

fn write_byte(byte: u8) {
    while read32(UART_BASE + UART_FR) & FR_TXFF != 0 {
        core::hint::spin_loop();
    }
    write32(UART_BASE + UART_DR, u32::from(byte));
}

fn write_str(string: &str) {
    for byte in string.bytes() {
        if byte == b'\n' {
            write_byte(b'\r');
        }
        write_byte(byte);
    }
}

fn read_byte() -> u8 {
    while read32(UART_BASE + UART_FR) & FR_RXFE != 0 {
        core::hint::spin_loop();
    }
    read32(UART_BASE + UART_DR) as u8
}

fn clear_rx() {
    while read32(UART_BASE + UART_FR) & FR_RXFE == 0 {
        let _ = read32(UART_BASE + UART_DR);
    }
}

fn flush() {
    while read32(UART_BASE + UART_FR) & FR_BUSY != 0 {
        core::hint::spin_loop();
    }
}

/// Receive a kernel at the firmware load address and enter it with the preserved device tree.
pub fn run(device_tree: *const u8) -> ! {
    init_uart();
    write_str("\nMiniLoad\n\n[ML] Requesting binary\n");
    clear_rx();
    for _ in 0..3 {
        write_byte(3);
    }

    let mut size = u32::from(read_byte());
    size |= u32::from(read_byte()) << 8;
    size |= u32::from(read_byte()) << 16;
    size |= u32::from(read_byte()) << 24;
    write_str("OK");

    let load_address = PAYLOAD_LOAD_ADDR as *mut u8;
    for offset in 0..size {
        unsafe { core::ptr::write_volatile(load_address.add(offset as usize), read_byte()) };
    }

    write_str("\n[ML] Loaded! Executing the payload now\n\n");
    flush();

    let kernel: extern "C" fn(*const u8) -> ! = unsafe { core::mem::transmute(PAYLOAD_LOAD_ADDR) };
    kernel(device_tree)
}
