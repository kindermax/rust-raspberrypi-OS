// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! BSP Memory Management.

//! The physical memory layout.
//!
//! The Raspberry's firmware copies the kernel binary to 0x8_0000. The preceding region will be used
//! as the boot core's stack.
//!
//! +---------------------------------------+
//! |                                       | 0x0
//! |                                       |                                ^
//! | Boot-core Stack                       |                                | stack
//! |                                       |                                | growth
//! |                                       |                                | direction
//! +---------------------------------------+
//! |                                       | code_start @ 0x8_0000
//! | .text                                 |
//! | .rodata                               |
//! | .got                                  |
//! |                                       |
//! +---------------------------------------+
//! |                                       | code_end_exclusive
//! | .data                                 |
//! | .bss                                  |
//! |                                       |
//! +---------------------------------------+
//! |                                       |
//! |                                       |
pub mod mmu;

use core::cell::UnsafeCell;

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

// Symbols from the linker script.
extern "Rust" {
    static __code_start: UnsafeCell<()>;
    static __code_end_exclusive: UnsafeCell<()>;
}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// The board's physical memory map.
#[rustfmt::skip]
pub(super) mod map {
    // TODO: this needs to be ported to rpi5 ?
    /// The inclusive end address of the memory map.
    ///
    /// End address + 1 must be power of two.
    ///
    /// # Note
    ///
    /// RPi3 and RPi4 boards can have different amounts of RAM. To make our code lean for
    /// educational purposes, we set the max size of the address space to 4 GiB regardless of board.
    /// This way, we can map the entire range that we need (end of MMIO for RPi4) in one take.
    ///
    /// However, making this trade-off has the downside of making it possible for the CPU to assert a
    /// physical address that is not backed by any DRAM (e.g. accessing an address close to 4 GiB on
    /// an RPi3 that comes with 1 GiB of RAM). This would result in a crash or other kind of error.
    pub const END_INCLUSIVE:       usize = 0xFFFF_FFFF;

    #[allow(dead_code)]
    pub const GPIO_OFFSET:         usize = 0x0020_0000;
    #[allow(dead_code)]
    pub const UART_OFFSET:         usize = 0x0020_1000;

    /// Physical devices.
    #[cfg(feature = "bsp_rpi3")]
    pub mod mmio {
        use super::*;

        pub const START:            usize =         0x3F000000;
        pub const GPIO_START:       usize = START + GPIO_OFFSET;
        pub const PL011_UART_START: usize = START + UART_OFFSET;
        pub const END_INCLUSIVE:    usize =         0x4000FFFF;
    }

    /// Physical devices.
    #[cfg(feature = "bsp_rpi4")]
    pub mod mmio {
        use super::*;

        pub const START:            usize =         0xFE000000;
        pub const GPIO_START:       usize = START + GPIO_OFFSET;
        pub const PL011_UART_START: usize = START + UART_OFFSET;
        pub const END_INCLUSIVE:    usize =         0xFF84FFFF;  //0xFEFFFFFF
    }

    /// Phsyical devices.
    #[cfg(feature = "bsp_rpi5")]
    pub mod mmio {
        /// Last two 64 KiB pages in the 4 GiB kernel address space, reserved for MMIO remapping.
        pub const UART_VIRT_PAGE_START: usize = 0xFFFE_0000;
        pub const GPIO_VIRT_PAGE_START: usize = 0xFFFF_0000;

        /// Virtual addresses handed to the device drivers after the MMU is enabled.
        pub const PL011_UART_START: usize = UART_VIRT_PAGE_START;
        pub const PL011_EARLY_UART_START: usize = UART_VIRT_PAGE_START + 0x1000;
        pub const GPIO_START: usize = GPIO_VIRT_PAGE_START;

        /// Physical addresses inherited from the firmware's RP1 setup.
        pub const PL011_UART_PHYS_START: usize = 0x1C_0003_0000;
        pub const GPIO_PHYS_START: usize = 0x1F_000D_0000;

        /// Physical address of BCM2712's dedicated debug UART (UART10).
        pub const PL011_EARLY_UART_PHYS_START: usize = 0x10_7D00_1000;

        #[cfg(not(feature = "early-uart"))]
        pub const ACTIVE_UART_PHYS_PAGE_START: usize = PL011_UART_PHYS_START;

        #[cfg(feature = "early-uart")]
        pub const ACTIVE_UART_PHYS_PAGE_START: usize =
            PL011_EARLY_UART_PHYS_START & !0xFFFF;
    }
}

// RPi 5 peripherals live above 4 GiB physically, but every address handed to a driver is a virtual
// address and must fit into the translation regime configured by `KernelAddrSpace`.
#[cfg(feature = "bsp_rpi5")]
const _: () = {
    assert!(map::mmio::PL011_UART_START <= map::END_INCLUSIVE);
    assert!(map::mmio::PL011_EARLY_UART_START <= map::END_INCLUSIVE);
    assert!(map::mmio::GPIO_START <= map::END_INCLUSIVE);

    assert!(
        map::mmio::GPIO_PHYS_START + (map::mmio::GPIO_START - map::mmio::GPIO_VIRT_PAGE_START)
            == map::mmio::GPIO_PHYS_START
    );
};

#[cfg(all(feature = "bsp_rpi5", not(feature = "early-uart")))]
const _: () = assert!(
    map::mmio::ACTIVE_UART_PHYS_PAGE_START
        + (map::mmio::PL011_UART_START - map::mmio::UART_VIRT_PAGE_START)
        == map::mmio::PL011_UART_PHYS_START
);

#[cfg(all(feature = "bsp_rpi5", feature = "early-uart"))]
const _: () = assert!(
    map::mmio::ACTIVE_UART_PHYS_PAGE_START
        + (map::mmio::PL011_EARLY_UART_START - map::mmio::UART_VIRT_PAGE_START)
        == map::mmio::PL011_EARLY_UART_PHYS_START
);

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

/// Start page address of the code segment.
///
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn code_start() -> usize {
    unsafe { __code_start.get() as usize }
}

/// Exclusive end page address of the code segment.
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn code_end_exclusive() -> usize {
    unsafe { __code_end_exclusive.get() as usize }
}
