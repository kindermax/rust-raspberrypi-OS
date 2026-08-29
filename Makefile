## SPDX-License-Identifier: MIT OR Apache-2.0
##
## Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

include ./common/docker.mk
include ./common/format.mk
include ./common/operating_system.mk
include ./common/serial.mk

##--------------------------------------------------------------------------------------------------
## Optional, user-provided configuration values
##--------------------------------------------------------------------------------------------------

# Default board
BSP ?= rpi5
SDCARD_DIR ?= /Volumes/BOOT

# Default to a serial device name that is common in Linux.
# Is RP1/GPIO UART (14/15 gpio)
# Best use: Chainloader and normal kernel development
DEV_SERIAL ?= /dev/tty.usbserial-0001
# Is the dedicated debug probe 3-pin connector labelled UART UART
# Best use: Earliest firmware/boot debugging
# DEV_SERIAL ?= /dev/cu.usbmodem112202
# Alias for debug probe UART
# DEV_SERIAL ?= /dev/tty.SLAB_USBtoUART

RPI5_EARLY_UART ?=

##--------------------------------------------------------------------------------------------------
## BSP-specific configuration values
##--------------------------------------------------------------------------------------------------
QEMU_MISSING_STRING = "This board is not yet supported for QEMU."

ifeq ($(BSP),rpi3)
    TARGET            = aarch64-unknown-none-softfloat
    NORMAL_KERNEL_BIN = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE = raspi3b
    QEMU_RELEASE_ARGS = -serial stdio -display none
    QEMU_TEST_ARGS    = $(QEMU_RELEASE_ARGS) -semihosting
    OBJDUMP_BINARY    = aarch64-none-elf-objdump
    NM_BINARY         = aarch64-none-elf-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a53
else ifeq ($(BSP),rpi4)
    TARGET            = aarch64-unknown-none-softfloat
    NORMAL_KERNEL_BIN = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE = raspi4b
    QEMU_RELEASE_ARGS = -serial stdio -display none
    QEMU_TEST_ARGS    = $(QEMU_RELEASE_ARGS) -semihosting
    OBJDUMP_BINARY    = aarch64-none-elf-objdump
    NM_BINARY         = aarch64-none-elf-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a72
else ifeq ($(BSP),rpi5)
    TARGET            = aarch64-unknown-none-softfloat
    NORMAL_KERNEL_BIN = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE =
    QEMU_RELEASE_ARGS = -serial stdio -display none
    QEMU_TEST_ARGS    = $(QEMU_RELEASE_ARGS) -semihosting
    OBJDUMP_BINARY    = aarch64-none-elf-objdump
    NM_BINARY         = aarch64-none-elf-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    OPENOCD_ARG       = -f ./cmsis-dap.cfg -f ./rpi5-openocd.cfg -c "adapter speed 5000"
    JTAG_BOOT_IMAGE   = ./X1_JTAG_boot/jtag_boot_rpi5.img
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a76
endif

##--------------------------------------------------------------------------------------------------
## Targets and Prerequisites
##--------------------------------------------------------------------------------------------------
KERNEL_MANIFEST   = kernel/Cargo.toml
CHAINLOADER_BIN   = chainloader8.img
KERNEL_BIN        = $(if $(CHAINLOADER),$(CHAINLOADER_BIN),$(NORMAL_KERNEL_BIN))

KERNEL_ELF      = target/$(TARGET)/release/kernel
TEST_BUILD_DIR  = target/test_build/$(BSP)
TEST_KERNEL_ELF = $(TEST_BUILD_DIR)/$(TARGET)/release/kernel
TEST_KERNEL_BIN = $(TEST_BUILD_DIR)/kernel8.img
HOST_TARGET     = $(shell rustc -vV | sed -n 's/^host: //p')
TEST_RUNNER     = $(shell pwd)/target/$(HOST_TARGET)/release/kernel_test_runner
# This parses cargo's dep-info file.
# https://doc.rust-lang.org/cargo/guide/build-cache.html#dep-info-files
KERNEL_ELF_DEPS = $(filter-out %: ,$(file < $(KERNEL_ELF).d)) $(KERNEL_MANIFEST)



##--------------------------------------------------------------------------------------------------
## Command building blocks
##--------------------------------------------------------------------------------------------------
RUSTFLAGS = $(RUSTC_MISC_ARGS)

RUSTFLAGS_PEDANTIC = $(RUSTFLAGS) \
    -D missing_docs

KERNEL_FEATURES = bsp_$(BSP)
ifeq ($(RPI5_EARLY_UART),1)
    KERNEL_FEATURES := $(KERNEL_FEATURES),early-uart
endif
ifdef CHAINLOADER
    KERNEL_FEATURES := $(KERNEL_FEATURES),chainloader
endif
FEATURES = --features $(KERNEL_FEATURES)
TEST_FEATURES = --no-default-features --features bsp_$(BSP),test_build
COMPILER_ARGS = --target=$(TARGET) \
    $(FEATURES) \
    --release

RUSTC_CMD   = cargo rustc $(COMPILER_ARGS) --manifest-path $(KERNEL_MANIFEST)
TEST_BOOT_RUSTC_CMD = cargo rustc                \
    --target=$(TARGET)                           \
    $(TEST_FEATURES)                             \
    --release                                    \
    --target-dir=$(TEST_BUILD_DIR)               \
    --manifest-path $(KERNEL_MANIFEST)
TEST_SELECTION = $(if $(TEST),--test $(TEST),--tests)
TEST_CMD = cargo test                            \
    --target=$(TARGET)                           \
    $(TEST_FEATURES)                             \
    --release                                    \
    --manifest-path $(KERNEL_MANIFEST)           \
    $(TEST_SELECTION)
DOC_CMD     = cargo doc $(COMPILER_ARGS)
HOST_CLIPPY_PACKAGES = kernel_test_runner
CLIPPY_KERNEL_CMD = cargo clippy $(COMPILER_ARGS) --manifest-path $(KERNEL_MANIFEST)
CLIPPY_HOST_CMD = cargo clippy --release --target=$(HOST_TARGET) \
    $(addprefix --package=,$(HOST_CLIPPY_PACKAGES))
OBJCOPY_CMD = rust-objcopy \
    --strip-all            \
    -O binary

EXEC_QEMU          = $(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE)

##------------------------------------------------------------------------------
## Dockerization
##------------------------------------------------------------------------------
DOCKER_CMD            = docker run -t --rm -v $(shell pwd):/work/tutorial -w /work/tutorial
DOCKER_CMD_INTERACT   = $(DOCKER_CMD) -i
DOCKER_ARG_DIR_COMMON = -v $(shell pwd)/common:/work/common
DOCKER_ARG_DEV        = --privileged -v /dev:/dev
DOCKER_ARG_NET        = --network host

# DOCKER_IMAGE defined in include file (see top of this file).
DOCKER_QEMU  = $(DOCKER_CMD_INTERACT) $(DOCKER_IMAGE)
DOCKER_TOOLS = $(DOCKER_CMD) $(DOCKER_IMAGE)
DOCKER_TEST  = $(DOCKER_CMD) $(DOCKER_ARG_DIR_COMMON) $(DOCKER_IMAGE)
DOCKER_GDB   = $(DOCKER_CMD_INTERACT) $(DOCKER_ARG_NET) $(DOCKER_IMAGE)

# Dockerize commands, which require USB device passthrough, only on Linux.
ifeq ($(shell uname -s),Linux)
    DOCKER_CMD_DEV = $(DOCKER_CMD_INTERACT) $(DOCKER_ARG_DEV)

    DOCKER_OPENOCD   = $(DOCKER_CMD_DEV) $(DOCKER_ARG_NET) $(DOCKER_IMAGE)
else ifeq ($(shell uname -s),Darwin)
    DOCKER_OPENOCD   =
    DOCKER_GDB =
else
    DOCKER_OPENOCD   = echo "Not yet supported on non-Linux systems."; \#
endif



##--------------------------------------------------------------------------------------------------
## Targets
##--------------------------------------------------------------------------------------------------
.PHONY: all FORCE chainboot copy-kernel-to-sdcard prepare-sdcard doc qemu miniterm clippy clean
.PHONY: readelf objdump nm check
.PHONY: test test_boot test_integration

all: $(KERNEL_BIN)

# Cargo can restore a feature-specific ELF from its cache with an older timestamp. Always run the
# cheap, cached Cargo and objcopy steps so switching build modes cannot leave a stale image behind.
FORCE:

##------------------------------------------------------------------------------
## Compile the kernel ELF
##------------------------------------------------------------------------------
$(KERNEL_ELF): FORCE $(KERNEL_ELF_DEPS)
	$(call color_header, "Compiling kernel ELF - $(BSP)")
	RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(RUSTC_CMD)

##------------------------------------------------------------------------------
## Generate the stripped kernel binary
##------------------------------------------------------------------------------
$(KERNEL_BIN): FORCE $(KERNEL_ELF)
	$(call color_header, "Generating stripped binary")
	@$(OBJCOPY_CMD) $(KERNEL_ELF) $(KERNEL_BIN)
	$(call color_progress_prefix, "Name")
	@echo $(KERNEL_BIN)
	$(call color_progress_prefix, "Size")
	$(call disk_usage_KiB, $(KERNEL_BIN))

copy-kernel-to-sdcard: $(KERNEL_BIN)
	$(call color_header, "Load to sdcard")
	cp $(KERNEL_BIN) $(SDCARD_DIR)/kernel8.img
	ls -lh $(SDCARD_DIR)
	diskutil unmount $(SDCARD_DIR)

prepare-sdcard:
	$(MAKE) BSP=$(BSP) CHAINLOADER=1 RPI5_EARLY_UART=1 all
	$(call color_header, "Prepare sdcard")
	ls -lh $(SDCARD_DIR)
	cp files/config.txt $(SDCARD_DIR)
	cp files/bcm2712-rpi-5-b.dtb $(SDCARD_DIR)
	cp files/fixup4.dat $(SDCARD_DIR)
	cp files/start4.elf $(SDCARD_DIR)
	cp $(CHAINLOADER_BIN) $(SDCARD_DIR)/kernel8.img
	ls -lh $(SDCARD_DIR)
	diskutil unmount $(SDCARD_DIR)


##------------------------------------------------------------------------------
## Generate the documentation
##------------------------------------------------------------------------------
doc:
	$(call color_header, "Generating docs")
	@$(DOC_CMD) --document-private-items --open

##------------------------------------------------------------------------------
## Run the kernel in QEMU
##------------------------------------------------------------------------------
ifeq ($(QEMU_MACHINE_TYPE),) # QEMU is not supported for the board.

qemu:
	$(call color_header, "$(QEMU_MISSING_STRING)")

else # QEMU is supported.

qemu: $(KERNEL_BIN)
	$(call color_header, "Launching QEMU")
	@$(DOCKER_QEMU) $(EXEC_QEMU) $(QEMU_RELEASE_ARGS) -kernel $(KERNEL_BIN)

endif

##------------------------------------------------------------------------------
## Connect to the target's serial
##------------------------------------------------------------------------------
miniterm:
	$(SCIP) $(SCIP_SERIAL_ARGS)

## Push the kernel to the real HW target
chainboot: $(NORMAL_KERNEL_BIN)
	$(call scip_upload,$(CHAINBOOT_PAYLOAD),payload)

##------------------------------------------------------------------------------
## Run clippy
##------------------------------------------------------------------------------
clippy: clippy_kernel clippy_host

clippy_kernel:
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(CLIPPY_KERNEL_CMD)

clippy_host:
	@$(CLIPPY_HOST_CMD)

##------------------------------------------------------------------------------
## Clean
##------------------------------------------------------------------------------
clean:
	rm -rf target $(NORMAL_KERNEL_BIN) $(CHAINLOADER_BIN)

##------------------------------------------------------------------------------
## Run readelf
##------------------------------------------------------------------------------
readelf: $(KERNEL_ELF)
	$(call color_header, "Launching readelf")
	@$(DOCKER_TOOLS) $(READELF_BINARY) --headers $(KERNEL_ELF)

##------------------------------------------------------------------------------
## Run objdump
##------------------------------------------------------------------------------
objdump: $(KERNEL_ELF)
	$(call color_header, "Launching objdump")
	@$(DOCKER_TOOLS) $(OBJDUMP_BINARY) --disassemble --demangle \
                --section .text   \
                --section .rodata \
                $(KERNEL_ELF) | rustfilt

##------------------------------------------------------------------------------
## Run nm
##------------------------------------------------------------------------------
nm: $(KERNEL_ELF)
	$(call color_header, "Launching nm")
	@$(DOCKER_TOOLS) $(NM_BINARY) --demangle --print-size $(KERNEL_ELF) | sort | rustfilt

##--------------------------------------------------------------------------------------------------
## Debugging targets
##--------------------------------------------------------------------------------------------------
.PHONY: jtagboot openocd openocd-local gdb gdb-opt0

##------------------------------------------------------------------------------
## Push the JTAG boot image to the real HW target
##------------------------------------------------------------------------------
jtagboot:
	$(call scip_upload,$(JTAG_BOOT_IMAGE),JTAG boot image)

##------------------------------------------------------------------------------
## Start OpenOCD session
##------------------------------------------------------------------------------
openocd:
	$(call color_header, "Launching OpenOCD")
	@$(DOCKER_OPENOCD) openocd $(OPENOCD_ARG)

openocd-local:
	$(call color_header, "Launching local OpenOCD with lldb")
	openocd -f ./cmsis-dap.cfg -f ./rpi5-openocd.cfg -c "adapter speed 5000"

##------------------------------------------------------------------------------
## Start GDB session
##------------------------------------------------------------------------------
gdb: RUSTC_MISC_ARGS += -C debuginfo=2
gdb-opt0: RUSTC_MISC_ARGS += -C debuginfo=2 -C opt-level=0
gdb gdb-opt0: $(KERNEL_ELF)
	$(call color_header, "Launching GDB")
	@$(DOCKER_GDB) aarch64-elf-gdb -x gdb_init -q $(KERNEL_ELF)

lldb: RUSTC_MISC_ARGS += -C debuginfo=2
lldb-opt0: RUSTC_MISC_ARGS += -C debuginfo=2 -C opt-level=0
lldb lldb-opt0: $(KERNEL_ELF)
	$(call color_header, "Launching LLDB")
	@$(DOCKER_GDB) lldb $(KERNEL_ELF)

##--------------------------------------------------------------------------------------------------
## Testing targets
##--------------------------------------------------------------------------------------------------

##------------------------------------------------------------------------------
## Run a deterministic boot smoke test in QEMU
##------------------------------------------------------------------------------
ifeq ($(QEMU_MACHINE_TYPE),) # QEMU is not supported for the board.

test_boot:
	$(call color_header, "$(QEMU_MISSING_STRING)")

else # QEMU is supported.

test_boot:
	$(call color_header, "Building QEMU boot test - $(BSP)")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(TEST_BOOT_RUSTC_CMD)
	@$(OBJCOPY_CMD) $(TEST_KERNEL_ELF) $(TEST_KERNEL_BIN)
	$(call color_header, "Running QEMU boot test - $(BSP)")
	$(EXEC_QEMU) $(QEMU_TEST_ARGS) -kernel $(TEST_KERNEL_BIN)

endif

##------------------------------------------------------------------------------
## Run the stable, harness-free integration test kernels in QEMU
##------------------------------------------------------------------------------
ifeq ($(QEMU_MACHINE_TYPE),) # QEMU is not supported for the board.

test_integration:
	$(call color_header, "$(QEMU_MISSING_STRING)")

else # QEMU is supported.

test_integration:
	$(call color_header, "Building kernel test runner")
	@cargo build --package kernel_test_runner --release --target $(HOST_TARGET)
	$(call color_header, "Running QEMU integration tests - $(BSP)")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)"                                  \
	CARGO_TARGET_AARCH64_UNKNOWN_NONE_SOFTFLOAT_RUNNER="$(TEST_RUNNER)" \
	KERNEL_TEST_QEMU="$(QEMU_BINARY)"                                  \
	KERNEL_TEST_QEMU_ARGS="-M $(QEMU_MACHINE_TYPE) $(QEMU_TEST_ARGS)"   \
	KERNEL_TEST_OBJCOPY="rust-objcopy"                                 \
	$(TEST_CMD)

endif

test: test_boot test_integration
