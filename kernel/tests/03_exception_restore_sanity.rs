// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2022-2023 Andre Richter <andre.o.richter@gmail.com>

//! Exception-context restoration sanity test kernel.

#![no_main]
#![no_std]

use core::arch::asm;
use libkernel::{bsp, cpu, exception, info, memory, test};

#[inline(never)]
fn nested_system_call() {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "svc #{svc_id}",
            svc_id = const test::EXCEPTION_RESTORE_SVC_ID,
            options(nomem, nostack, preserves_flags)
        );
    }

    #[cfg(not(target_arch = "aarch64"))]
    cpu::qemu_exit_failure()
}

#[no_mangle]
unsafe fn kernel_init() -> ! {
    use memory::mmu::interface::MMU;

    exception::handling_init();
    memory::mmu::mmu()
        .enable_mmu_and_caching()
        .expect("MMU initialization failed");
    bsp::driver::qemu_bring_up_console();

    info!("Making a test system call");
    nested_system_call();
    info!("Returned from the system call");

    cpu::qemu_exit_success()
}
