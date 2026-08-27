## SPDX-License-Identifier: MIT OR Apache-2.0
##
## Shared host-side serial console and MiniPush configuration.

SERIAL_BAUD ?= 115200
SCIP ?= scip
SCIP_SERIAL_ARGS ?= "$(DEV_SERIAL)" $(SERIAL_BAUD) 8 N 1 N
CHAINBOOT_PAYLOAD ?= $(KERNEL_BIN)

define scip_upload
	@test -f "$(1)" || { \
		echo "Missing $(2): $(1)"; \
		exit 1; \
	}
	$(SCIP) --binfile "$(1)" $(SCIP_SERIAL_ARGS)
endef
