// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! BSP Memory Management Unit.

use super::map as memory_map;
use crate::memory::mmu::*;
use core::ops::RangeInclusive;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// The kernel's address space defined by this BSP.
pub type KernelAddrSpace = AddressSpace<{ memory_map::END_INCLUSIVE + 1 }>;

#[cfg(not(feature = "bsp_rpi5"))]
const NUM_MEM_RANGES: usize = 2;

#[cfg(feature = "bsp_rpi5")]
const NUM_MEM_RANGES: usize = 3;

/// The virtual memory layout.
///
/// The layout must contain only special ranges, aka anything that is _not_ normal cacheable DRAM.
/// It is agnostic of the paging granularity that the architecture's MMU will use.
#[cfg(not(feature = "bsp_rpi5"))]
pub static LAYOUT: KernelVirtualLayout<NUM_MEM_RANGES> = KernelVirtualLayout::new(
    memory_map::END_INCLUSIVE,
    [
        TranslationDescriptor {
            name: "Kernel code and RO data",
            virtual_range: code_range_inclusive,
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::CacheableDRAM,
                acc_perms: AccessPermissions::ReadOnly,
                execute_never: false,
            },
        },
        TranslationDescriptor {
            name: "Device MMIO",
            virtual_range: mmio_range_inclusive,
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        },
    ],
);

#[cfg(feature = "bsp_rpi5")]
/// The RPi 5 virtual memory layout, including sub-4-GiB virtual MMIO windows.
pub static LAYOUT: KernelVirtualLayout<NUM_MEM_RANGES> = KernelVirtualLayout::new(
    memory_map::END_INCLUSIVE,
    [
        TranslationDescriptor {
            name: "Kernel code and RO data",
            virtual_range: code_range_inclusive,
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::CacheableDRAM,
                acc_perms: AccessPermissions::ReadOnly,
                execute_never: false,
            },
        },
        TranslationDescriptor {
            name: "UART MMIO remap",
            virtual_range: uart_range_inclusive,
            physical_range_translation: Translation::Offset(
                memory_map::mmio::ACTIVE_UART_PHYS_PAGE_START,
            ),
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        },
        TranslationDescriptor {
            name: "GPIO MMIO remap",
            virtual_range: gpio_range_inclusive,
            physical_range_translation: Translation::Offset(memory_map::mmio::GPIO_PHYS_START),
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        },
    ],
);

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

fn code_range_inclusive() -> RangeInclusive<usize> {
    // Notice the subtraction to turn the exclusive end into an inclusive end.
    #[allow(clippy::range_minus_one)]
    RangeInclusive::new(super::code_start(), super::code_end_exclusive() - 1)
}

#[cfg(not(feature = "bsp_rpi5"))]
fn mmio_range_inclusive() -> RangeInclusive<usize> {
    RangeInclusive::new(memory_map::mmio::START, memory_map::mmio::END_INCLUSIVE)
}

#[cfg(feature = "bsp_rpi5")]
fn uart_range_inclusive() -> RangeInclusive<usize> {
    RangeInclusive::new(
        memory_map::mmio::UART_VIRT_PAGE_START,
        memory_map::mmio::UART_VIRT_PAGE_START + 0xFFFF,
    )
}

#[cfg(feature = "bsp_rpi5")]
fn gpio_range_inclusive() -> RangeInclusive<usize> {
    RangeInclusive::new(
        memory_map::mmio::GPIO_VIRT_PAGE_START,
        memory_map::mmio::GPIO_VIRT_PAGE_START + 0xFFFF,
    )
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Return a reference to the virtual memory layout.
pub fn virt_mem_layout() -> &'static KernelVirtualLayout<NUM_MEM_RANGES> {
    &LAYOUT
}

/// Validate that RPi5 driver addresses resolve through the configured MMIO descriptors.
#[cfg(feature = "bsp_rpi5")]
pub fn validate_layout() -> Result<(), &'static str> {
    let uart_virtual = if cfg!(feature = "early-uart") {
        memory_map::mmio::PL011_EARLY_UART_START
    } else {
        memory_map::mmio::PL011_UART_START
    };
    let uart_physical = if cfg!(feature = "early-uart") {
        memory_map::mmio::PL011_EARLY_UART_PHYS_START
    } else {
        memory_map::mmio::PL011_UART_PHYS_START
    };

    let (mapped_uart, uart_attributes) = virt_mem_layout().virt_addr_properties(uart_virtual)?;
    if mapped_uart != uart_physical
        || !matches!(uart_attributes.mem_attributes, MemAttributes::Device)
    {
        return Err("RPi5 UART MMIO mapping is invalid");
    }

    let (mapped_gpio, gpio_attributes) =
        virt_mem_layout().virt_addr_properties(memory_map::mmio::GPIO_START)?;
    if mapped_gpio != memory_map::mmio::GPIO_PHYS_START
        || !matches!(gpio_attributes.mem_attributes, MemAttributes::Device)
    {
        return Err("RPi5 GPIO MMIO mapping is invalid");
    }

    Ok(())
}
