# Proposal: `make probe-reset`

Status: proposed

## Summary

Add an RPi 5-only `make probe-reset` target that halts CPU0 through the Raspberry Pi Debug Probe,
validates a reset trampoline in the running kernel, and resumes at that trampoline. The trampoline
invokes the firmware's PSCI `SYSTEM_RESET` service, causing a full firmware reboot back into the
SD-card chainloader.

The target must not inject instructions into arbitrary RAM, write undocumented reset-controller
registers, or assume that the locally built ELF matches the kernel running on the board.

## Motivation and observed behavior

The Raspberry Pi Debug Probe can control the BCM2712 through SWD when its target cable is connected
to port **D**. Port **U** remains the UART connection. The repository's
[`rpi5-openocd.cfg`](../../rpi5-openocd.cfg) successfully discovers all four Cortex-A76 cores and
supports halt, register inspection, and resume.

It cannot currently request a native OpenOCD reset:

- CMSIS-DAP reports `nRESET = 0`; the three-wire D connection has only SWCLK, SWDIO, and ground.
- `reset run` fails with `bcm2712.cpu0: how to reset?` because the target configuration has neither
  an SRST signal nor a `reset-assert` handler.
- `soft_reset_halt` reports that the AArch64 target does not support it.

BCM2712 advertises PSCI 1.0 with the SMC conduit in the upstream
[Raspberry Pi Linux device tree][bcm2712-psci]. A hardware proof of concept set `x0` to the PSCI
`SYSTEM_RESET` function ID (`0x84000009`) and executed `smc #0`. OpenOCD disconnected immediately;
after reconnecting, CPU0 was at EL2 in the relocated chainloader. A second run with `make chainboot`
waiting produced firmware logs, a fresh MiniLoad request, a successful upload, and a normal
MMU-enabled kernel boot.

The proof of concept also demonstrated why arbitrary RAM injection should not become the product:
an attempted MEM-AP write into executable padding did not stick, and executing that address caused
an undefined-instruction exception. The later successful attempt was gated on exact readback and
used writable RAM while translation was disabled.

## Proposed design

### 1. Link a dedicated reset trampoline

Add an AArch64 assembly routine with a stable exported symbol, for example
`__debug_psci_system_reset`:

```asm
.section .text.debug_reset, "ax"
.global __debug_psci_system_reset
.type __debug_psci_system_reset, %function
__debug_psci_system_reset:
    msr     daifset, #0xf
    movz    x0, #0x0009
    movk    x0, #0x8400, lsl #16
    smc     #0
1:
    wfe
    b       1b
```

Both [`kernel.ld`](../../kernel/src/bsp/raspberrypi/kernel.ld) and
[`chainloader.ld`](../../kernel/src/bsp/raspberrypi/chainloader.ld) should retain the section with
`KEEP(*(.text.debug_reset))`. Retention is required because no Rust call site references the
debugger-only symbol.

The routine sets its own PSCI function ID. The host must not depend on a previously cached value of
`x0`. Masking exceptions prevents an interrupt from redirecting execution between resume and the
SMC. The loop is a defensive fallback for firmware that unexpectedly returns from `SYSTEM_RESET`.

### 2. Resolve the symbol from the local ELF

`make probe-reset` should depend on the RPi 5 kernel ELF and resolve the trampoline using
`rust-nm`, which is already available with the `cargo-binutils` tooling used for `rust-objcopy`:

```text
rust-nm --defined-only target/aarch64-unknown-none-softfloat/debug/kernel
```

The target must fail before starting OpenOCD if the symbol is missing or appears more than once.
The symbol address is not treated as proof that the running image matches the ELF.

### 3. Validate target memory before changing `PC`

Add a small OpenOCD Tcl script, for example `rpi5-probe-reset.cfg`, that receives the resolved
address and performs this sequence:

1. `init` the CMSIS-DAP/SWD session.
2. Halt `bcm2712.cpu0` with a bounded timeout.
3. Read the trampoline words through the CPU target.
4. Compare every word with the expected instruction sequence.
5. On mismatch, resume at the original `PC`, report a stale/mismatched ELF, and fail without reset.
6. On a match, resume CPU0 at `__debug_psci_system_reset` and shut down OpenOCD.

Opcode validation is the safety boundary. It prevents a locally rebuilt ELF from sending execution
to unrelated code in an older kernel that is still running on the board. The Tcl script should own
the halt/resume cleanup so every failure after a successful halt either resumes the original state
or clearly reports that it could not do so.

### 4. Add the Make target

The Makefile target should be RPi 5-only and reuse `OPENOCD_ARG` and `DOCKER_OPENOCD`, preserving
the repository's current Linux container and macOS local-OpenOCD behavior.

Expected interface:

```bash
make probe-reset
DEV_SERIAL=/dev/tty.usbserial-0001 make chainboot
```

`probe-reset` should print that a successful reset discards the RAM-loaded kernel and that MiniLoad
will wait for the next upload. Automatically combining reset and chainboot is intentionally outside
the first implementation: keeping SWD reset and UART upload separate makes failures attributable to
one transport at a time.

## Failure behavior

The command must exit non-zero without attempting the SMC when:

- `BSP` is not `rpi5`;
- `openocd` or `rust-nm` is unavailable;
- the CMSIS-DAP probe is missing or the cable is not connected to D;
- SWD cannot enumerate the BCM2712 DAP;
- CPU0 cannot be halted within the timeout;
- the reset symbol cannot be resolved uniquely; or
- the target opcodes do not match the ELF.

After any failure that occurs while CPU0 is halted, the script must attempt to resume the original
`PC`. It must never fall back to guessed watchdog, PMIC, RP1, or reset-controller writes.

## Limitations

- This is a cooperative architectural reset path, not a hardware reset line. It cannot recover a
  target whose debug port, CPU0, exception level, instruction fetch, or trusted firmware is
  unavailable.
- The first implementation targets the current identity-mapped RPi 5 kernel and PSCI firmware boot
  flow. RPi 3 and RPi 4 require separate validation before enabling the target for those BSPs.
- A successful reset destroys the uploaded RAM kernel and returns to the SD-card boot flow.
- If the SD card does not contain the chainloader, reset starts whatever `kernel8.img` is installed
  there instead.

## Verification plan

### Static checks

1. Build normal and `CHAINLOADER=1` RPi 5 ELFs.
2. Verify that `rust-nm` reports exactly one `__debug_psci_system_reset` symbol in each ELF.
3. Disassemble the symbol and compare its complete instruction sequence with the Tcl expectations.
4. Confirm `git diff --check`, formatting, and the existing RPi 5 build/test targets pass.

### Hardware checks

1. Boot the normal kernel and confirm UART echo and `MMU online`.
2. Start `DEV_SERIAL=/dev/tty.usbserial-0001 make chainboot` so the uploader is waiting.
3. Run `make probe-reset` with the probe connected through D.
4. Confirm firmware boot logs, `MiniLoad`, successful upload, and a second normal kernel boot.
5. Repeat the reset/upload cycle to rule out one-time debugger state.
6. Deliberately supply a wrong expected opcode or an ELF from another build; verify the target is
   resumed without resetting and the command reports an image mismatch.
7. Disconnect D and verify the command fails cleanly without affecting the running kernel.

## Acceptance criteria

- `make probe-reset` resets an MMU-enabled RPi 5 kernel into the SD-card chainloader without a power
  cycle.
- No target RAM or MMIO is modified before the trampoline has been validated.
- A stale local ELF cannot redirect execution to an unvalidated address.
- All post-halt error paths attempt to restore execution at the original `PC`.
- Two consecutive probe-reset/chainboot cycles succeed on hardware.
- The README documents the command and its requirement to use probe port D.

[bcm2712-psci]: https://github.com/raspberrypi/linux/blob/rpi-6.12.y/arch/arm64/boot/dts/broadcom/bcm2712.dtsi#L152-L155
