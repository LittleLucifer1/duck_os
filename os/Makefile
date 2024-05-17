# Building
TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
KERNEL_BIN := $(KERNEL_ELF).bin
DISASM_TMP := target/$(TARGET)/$(MODE)/asm
# FS_IMG_FAT32 := ../fat32.img
FS_IMG_FAT32 := ../sdcard.img
# FS_IMG := ../fs.img
FS_IMG := ../sdcard.img
KERNEL_FEATURE :=
TEST_DIR := ../testcases/riscv64
CARGO_CONFIG := ./cargo_config

CPUS = 2
PRELIMINARY = 1
TEST ?= 

# Building mode argument
ifeq ($(MODE), release)
	MODE_ARG := --release
endif

# BOARD
BOARD := qemu
SBI ?= rustsbi
BOOTLOADER := ../bootloader/$(SBI)-$(BOARD).bin

# KERNEL ENTRY
KERNEL_ENTRY_PA := 0x80200000

# Binutils
OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

# Disassembly
DISASM ?= -d

build: env $(KERNEL_BIN) fs-img

fs-img:
	@rm -f $(FS_IMG)
	@dd if=/dev/zero of=$(FS_IMG) bs=512 count=524288
	@mkfs.vfat -F 32 $(FS_IMG)

# build-fat32: env $(KERNEL_BIN) fat32-img
build-fat32: $(KERNEL_BIN)

fat32-img: 
	@rm -f $(FS_IMG_FAT32)
	@dd if=/dev/zero of=$(FS_IMG_FAT32) bs=512 count=524288
	@mkfs.vfat -F 32 $(FS_IMG_FAT32)
	@echo "make fatfs img"
	@-mkdir -p mnt
	@sudo mount -t vfat -o user,umask=000,utf8=1 --source $(FS_IMG_FAT32) --target mnt
	@-cp -r $(TEST_DIR)/* mnt
	@sudo umount mnt
	@rm -rf mnt
	@sudo chmod -R 777 $(FS_IMG_FAT32)
	@echo "finish packing fatfs img"
	

env:
	(rustup target list | grep "riscv64gc-unknown-none-elf (installed)") || rustup target add $(TARGET)
	cargo install cargo-binutils
	rustup component add rust-src
	rustup component add llvm-tools-preview

$(KERNEL_BIN): kernel
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $@

user-bin: 
	@echo "build user bin"
	@cd ../user && make build PRELIMINARY=$(PRELIMINARY) TEST=$(TEST)
	@echo "build user finished"

kernel:
	@echo Platform: $(BOARD)
# ifeq ($(KERNEL_FEATURE), )
# 	@cargo build $(MODE_ARG) --offline
# else
# 	@cargo build $(MODE_ARG) --feature $(KERNEL_FEATURE)
# endif
	@echo "build kernel bin finished"
	
clean:
	@cargo clean

disasm:
ifeq ("$(wildcard $(KERNEL_ELF))", "")
	@echo "No kernel elf file."
	@exit 1
endif
	@$(OBJDUMP) $(DISASM) $(KERNEL_ELF) | less

disasm-vim: kernel
	@$(OBJDUMP) $(DISASM) $(KERNEL_ELF) > $(DISASM_TMP)
	@vim $(DISASM_TMP)
	@rm $(DISASM_TMP)

run: run-fat32

all :build-fat32
	@cp $(KERNEL_BIN) ../kernel_qemu
	@cp $(BOOTLOADER) ../sbi-qemu

run-inner: build
	@echo "start to run bare kernel in qemu"
	@qemu-system-riscv64 \
		-machine virt \
		-nographic \
		-bios $(BOOTLOADER) \
		-kernel $(KERNEL_BIN) \
		-m 128M \
		-smp $(CPUS) \
		-drive file=$(FS_IMG),if=none,format=raw,id=x0 \
		-device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

# -device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA)


run-fat32: build-fat32
	@echo "start to run kernel in qemu"
	@qemu-system-riscv64 \
		-machine virt \
		-nographic \
		-bios $(BOOTLOADER) \
		-kernel $(KERNEL_BIN) \
		-m 128M \
		-smp $(CPUS) \
		-drive file=$(FS_IMG_FAT32),if=none,format=raw,id=x0 \
		-device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

debug: build
	@tmux new-session -d \
		"qemu-system-riscv64 -machine virt -nographic -bios $(BOOTLOADER) -device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA) -s -S" && \
		tmux split-window -h "riscv64-unknown-elf-gdb -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'" && \
		tmux -2 attach-session -d

mode: 
	@MODE=build

gdbserver: build-fat32
	@qemu-system-riscv64 \
		-machine virt \
		-nographic \
		-bios $(BOOTLOADER) \
		-kernel $(KERNEL_BIN) \
		-m 128M \
		-smp $(CPUS) \
		-drive file=$(FS_IMG_FAT32),if=none,format=raw,id=x0 \
		-device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-s -S

gdbclient:
	@riscv64-unknown-elf-gdb -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'

.PHONY: build env kernel clean all disasm disasm-vim run-inner gdbserver gdbclient cargo_config run-local
