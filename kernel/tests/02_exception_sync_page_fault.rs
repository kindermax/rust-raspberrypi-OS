// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2019-2023 Andre Richter <andre.o.richter@gmail.com>

//! A synchronous page fault must reach the kernel panic path.

#![no_main]
#![no_std]

use libkernel::{bsp, cpu, exception, info, memory, test};

const PAGE_FAULT_ADDRESS: usize = 9 * 1024 * 1024 * 1024;

#[no_mangle]
unsafe fn kernel_init() -> ! {
    use memory::mmu::interface::MMU;

    exception::handling_init();
    memory::mmu::mmu()
        .enable_mmu_and_caching()
        .expect("MMU initialization failed");
    bsp::driver::qemu_bring_up_console();

    info!("Causing a page fault by reading address 9 GiB");
    assert!(
        bsp::memory::mmu::virt_mem_layout()
            .virt_addr_properties(PAGE_FAULT_ADDRESS)
            .is_err(),
        "page-fault probe address is mapped"
    );
    test::expect_panic();
    core::ptr::read_volatile(PAGE_FAULT_ADDRESS as *const u64);

    cpu::qemu_exit_failure()
}
