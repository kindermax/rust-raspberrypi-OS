// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! Architectural processor code.
//!
//! # Orientation
//!
//! Since arch modules are imported into generic modules using the path attribute, the path of this
//! file is:
//!
//! crate::cpu::arch_cpu

use aarch64_cpu::asm;

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------
pub use asm::nop;

/// Pause execution on the core.
#[inline(always)]
pub fn wait_forever() -> ! {
    loop {
        asm::wfe()
    }
}
