    .section .text.entry
    .globl _start
_start:
    # hart_id is in a0, a0 --> tp
    # pc = 0x8020_0000

    # 1.According to hart_id, set sp to the top
    la sp, boot_stack_top
    mv tp, a0
    slli t0, tp, 16 # 内核栈大小size: (4096 * 16)
    sub sp, sp, t0
    
    # 2.set table & satp
    # satp: 8 << 60 | boot_pagetable
    la t0, boot_pagetable
    li t1, 8 << 60
    srli t0, t0, 12
    or t0, t0, t1
    csrw satp, t0
    sfence.vma
    # 3.reset stack
    # the offset is 0xffff_ffff_0000_0000
    li t1, 0xffffffff00000000
    add sp, sp, t1
    # 4.jump to rust_main
    la t0, rust_main
    add t0, t0, t1
    jr t0

    .section .bss.stack

    .globl boot_stack_lower
boot_stack_lower:
# 内核栈总大小：最多8个核，每个核有15页，再加上一个guard page, 总共16页——64kb
    .space 4096 * 16 * 8  

    .globl boot_stack_top
boot_stack_top:

    .section .data
    .align 12
boot_pagetable:
    # 0x0000_0000_8000_0000 ---> 0x0000_0000_8000_0000
    # 0xffff_ffff_8000_0000 ---> 0x0000_0000_8000_0000
    .quad 0
    .quad 0
    .quad (0x80000 << 10) | 0xcf #VRWXAD vpn L2 = 0b10
    .zero 8 * 507
    .quad (0x80000 << 10) | 0xcf # vpn L2 = 0b1_1111_1110
    .quad 0