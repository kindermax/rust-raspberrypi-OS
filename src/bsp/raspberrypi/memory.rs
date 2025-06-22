// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! BSP Memory Management.

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// The board's physical memory map.
#[rustfmt::skip]
pub(super) mod map {

    #[allow(dead_code)]
    pub const GPIO_OFFSET:         usize = 0x0020_0000;
    #[allow(dead_code)]
    pub const UART_OFFSET:         usize = 0x0020_1000;

    /// Physical devices.
    #[cfg(feature = "bsp_rpi3")]
    pub mod mmio {
        use super::*;

        pub const START:            usize =         0x3F00_0000;
        pub const GPIO_START:       usize = START + GPIO_OFFSET;
        pub const PL011_UART_START: usize = START + UART_OFFSET;
    }

    /// Physical devices.
    #[cfg(feature = "bsp_rpi4")]
    pub mod mmio {
        use super::*;

        pub const START:            usize =         0xFE00_0000;
        pub const GPIO_START:       usize = START + GPIO_OFFSET;
        pub const PL011_UART_START: usize = START + UART_OFFSET;
    }

    /// Phsyical devices.
    #[cfg(feature = "bsp_rpi5")]
    pub mod mmio {
        // use super::*;

        // pub const START:            usize =         0xFC00_0000;
        // pub const GPIO_START:       usize = START + GPIO_OFFSET;
        // pub const PL011_UART_START: usize = START + UART_OFFSET;

        // peripheral base address
        #[allow(dead_code)]
        pub const START:            usize =            0x107c000000;
        pub const GPIO_START:       usize =            0x1f000d0000;
        pub const PL011_UART_START: usize =         0x1c00030000; // w/o pcie
        // pub const PL011_UART_START: usize =         0x1f00030000; // w/ pcie
        pub const PL011_EARLY_UART_START: usize = 0x107d001000;
    }
}
