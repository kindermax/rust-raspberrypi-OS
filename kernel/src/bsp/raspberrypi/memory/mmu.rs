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

//--------------------------------------------------------------------------------------------------
// Testing
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use test_macros::kernel_test;

    /// Check alignment of the kernel's virtual memory layout sections.
    #[kernel_test]
    fn virt_mem_layout_sections_are_64KiB_aligned() {
        const SIXTYFOUR_KIB: usize = 65536;

        for i in LAYOUT.inner().iter() {
            let start: usize = *(i.virtual_range)().start();
            let end: usize = *(i.virtual_range)().end() + 1;

            assert_eq!(start % SIXTYFOUR_KIB, 0);
            assert_eq!(end % SIXTYFOUR_KIB, 0);
            assert!(end >= start);
        }
    }

    /// Ensure the kernel's virtual memory layout is free of overlaps.
    #[kernel_test]
    fn virt_mem_layout_has_no_overlaps() {
        let layout = virt_mem_layout().inner();

        for (i, first) in layout.iter().enumerate() {
            for second in layout.iter().skip(i + 1) {
                let first_range = first.virtual_range;
                let second_range = second.virtual_range;

                assert!(!first_range().contains(second_range().start()));
                assert!(!first_range().contains(second_range().end()));
                assert!(!second_range().contains(first_range().start()));
                assert!(!second_range().contains(first_range().end()));
            }
        }
    }

    /// Ensure RPi 5 drivers use virtual MMIO addresses that resolve to the intended peripherals.
    #[cfg(feature = "bsp_rpi5")]
    #[kernel_test]
    fn rpi5_driver_mmio_addresses_are_mapped() {
        let uart_virt_addr = if cfg!(feature = "early-uart") {
            memory_map::mmio::PL011_EARLY_UART_START
        } else {
            memory_map::mmio::PL011_UART_START
        };
        let uart_phys_addr = if cfg!(feature = "early-uart") {
            memory_map::mmio::PL011_EARLY_UART_PHYS_START
        } else {
            memory_map::mmio::PL011_UART_PHYS_START
        };

        let (mapped_uart_addr, uart_attrs) = virt_mem_layout()
            .virt_addr_properties(uart_virt_addr)
            .unwrap();
        assert_eq!(mapped_uart_addr, uart_phys_addr);
        assert!(matches!(uart_attrs.mem_attributes, MemAttributes::Device));

        let (mapped_gpio_addr, gpio_attrs) = virt_mem_layout()
            .virt_addr_properties(memory_map::mmio::GPIO_START)
            .unwrap();
        assert_eq!(mapped_gpio_addr, memory_map::mmio::GPIO_PHYS_START);
        assert!(matches!(gpio_attrs.mem_attributes, MemAttributes::Device));

        // Keep the exception tutorial's page-fault address outside the translation regime.
        let nine_gib = 9 * 1024 * 1024 * 1024;
        assert!(virt_mem_layout().virt_addr_properties(nine_gib).is_err());
    }
}
