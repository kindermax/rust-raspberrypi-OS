// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! PL011 UART driver.
//!
//! # Resources
//!
//! - <https://datasheets.raspberrypi.com/rp1/rp1-peripherals.pdf>
//! - <https://developer.arm.com/documentation/ddi0183/latest>

use crate::{
    bsp::device_driver::common::MMIODerefWrapper, console, cpu, driver, synchronization,
    synchronization::NullLock,
};
use core::{
    fmt::{self, Write},
    ptr,
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

// BCM2712 UART register addresses
const BCM2712_UART_BASE: u64 = 0x10_7d00_1000;
const BCM2712_UART_DR: *mut u32 = BCM2712_UART_BASE as *mut u32;
const BCM2712_UART_FLAG: *mut u32 = (BCM2712_UART_BASE + 0x18) as *mut u32;
// Flag Register bits
const UART_FR_BUSY: u32 = 1 << 3; // UART busy
const UART_FR_RXFE: u32 = 1 << 4; // RX FIFO empty
const UART_FR_TXFF: u32 = 1 << 5; // TX FIFO full

#[derive(PartialEq)]
enum BlockingMode {
    Blocking,
    NonBlocking,
}

struct PL011EarlyUartInner {}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

pub trait Uart {
    fn write_char(&self, c: char);
    fn read_char(&self, mode: BlockingMode) -> Option<char>;
    fn write_str(&self, s: &str);
}

/// Representation of the UART.
pub struct PL011EarlyUart {
    // TODO: actually there is no need for lock here ?
    inner: NullLock<PL011EarlyUartInner>,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

impl PL011EarlyUartInner {
    /// Create an instance.
    pub const fn new() -> Self {
        Self {}
    }

    /// Pi5 UART initialization - very basic from early_uart
    pub unsafe fn init(&self) -> Result<(), &'static str> {
        // No initialization needed for early UART - already configured by bootloader
        Ok(())
    }

    // Write a raw character without CRLF conversion
    fn write_char_raw(&self, c: char) {
        unsafe {
            // Wait for TX FIFO not full
            while ptr::read_volatile(BCM2712_UART_FLAG) & UART_FR_TXFF != 0 {
                cpu::nop();
            }

            // Write character
            ptr::write_volatile(BCM2712_UART_DR, c as u32);
        }
    }

    /// Block execution until the last buffered character has been physically put on the TX wire.
    fn flush(&self) {
        // Spin until the busy bit is cleared.
        unsafe {
            while ptr::read_volatile(BCM2712_UART_FLAG) & UART_FR_BUSY != 0 {
                cpu::nop();
            }
        }
    }
}

impl Uart for PL011EarlyUartInner {
    // // Write a string
    fn write_str(&self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }

    // Write a character - direct from early_uart
    fn write_char(&self, c: char) {
        // Handle newline conversion to CRLF
        if c == '\n' {
            self.write_char_raw('\r');
        }

        self.write_char_raw(c);
    }

    fn read_char(&self, mode: BlockingMode) -> Option<char> {
        unsafe {
            // Check if RX FIFO is empty
            if ptr::read_volatile(BCM2712_UART_FLAG) & UART_FR_RXFE != 0 {
                // immediately return in non-blocking mode.
                if mode == BlockingMode::NonBlocking {
                    return None;
                }

                // Otherwise, wait until a char was received.
                while ptr::read_volatile(BCM2712_UART_FLAG) & UART_FR_RXFE != 0 {
                    cpu::nop();
                }
            }

            // Read character
            let data = ptr::read_volatile(BCM2712_UART_DR);
            Some((data & 0xFF) as u8 as char)
        }
    }

}

/// Implementing `core::fmt::Write` enables usage of the `format_args!` macros, which in turn are
/// used to implement the `kernel`'s `print!` and `println!` macros. By implementing `write_str()`,
/// we get `write_fmt()` automatically.
///
/// The function takes an `&mut self`, so it must be implemented for the inner struct.
///
/// See [`src/print.rs`].
///
/// [`src/print.rs`]: ../../print/index.html
impl fmt::Write for PL011EarlyUartInner {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Uart::write_str(self, s);
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        Uart::write_char(self, c);
        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl PL011EarlyUart {
    pub const COMPATIBLE: &'static str = "BCM PL011 UART";

    pub const fn new() -> Self {
        Self {
            inner: NullLock::new(PL011EarlyUartInner::new()),
        }
    }
}

impl fmt::Write for PL011EarlyUart {
    // Write a string
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.inner.lock(|inner| inner.write_str(s));

        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        self.inner.lock(|inner| inner.write_char(c));

        Ok(())
    }
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
use synchronization::interface::Mutex;

impl driver::interface::DeviceDriver for PL011EarlyUart {
    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }

    unsafe fn init(&self) -> Result<(), &'static str> {
        self.inner.lock(|inner| inner.init());

        Ok(())
    }
}

impl console::interface::Write for PL011EarlyUart {
    /// Passthrough of `args` to the `core::fmt::Write` implementation, but guarded by a Mutex to
    /// serialize access.
    fn write_char(&self, c: char) {
        self.inner.lock(|inner| inner.write_char(c));
    }

    fn write_fmt(&self, args: core::fmt::Arguments) -> fmt::Result {
        // Fully qualified syntax for the call to `core::fmt::Write::write_fmt()` to increase
        // readability.
        self.inner.lock(|inner| fmt::Write::write_fmt(inner, args))
    }

    fn flush(&self) {
        // Spin until TX FIFO empty is set.
        self.inner.lock(|inner| inner.flush());
    }
}

impl console::interface::Read for PL011EarlyUart {
    fn read_char(&self) -> char {
        self.inner
            .lock(|inner| Uart::read_char(inner, BlockingMode::Blocking).unwrap())
    }

    fn clear_rx(&self) {
        // Read from the RX FIFO until it is indicating empty.
        while self
            .inner
            .lock(|inner| Uart::read_char(inner, BlockingMode::NonBlocking))
            .is_some()
        {}
    }
}

impl console::interface::Statistics for PL011EarlyUart {
    fn chars_written(&self) -> usize {
        0
    }

    fn chars_read(&self) -> usize {
        0
    }
}

impl console::interface::All for PL011EarlyUart {}
