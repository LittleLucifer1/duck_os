.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text
    .globl __alltraps
    .globl __restore
    .align 2 # stvec中的BASE必须保证是4字节对齐！因为BASE中表示的地址默认后两位为0
# TODO：暂时没有考虑中断嵌套的问题，如果 U -> S -> S 那会发生什么？
__alltraps:
    # 不能使用spp来判断，因为此时没有空余的寄存器可以使用
    # 于是这里把sp看作有符号数来判断，如果是在用户空间，则为正数，否则为负数。
    # 用户态：则交换sp sscratch； 内核态：不用交换
    bgtz sp, __user_to_kernel
    sd tp, -1*8(sp)

__trap_entry:
    # sp -> kernel stack
    addi sp, sp, -35*8
    sd x1, 1*8(sp)
    .set n, 3
    .rept 29
        SAVE_GP %n
        .set n, n+1
    .endr
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    csrr t2, sscratch
    sd t2, 2*8(sp)
    ld tp, 34*8(sp)
    mv a0, sp
    call trap_handler

__restore:
    # 从内核到用户态，或者从内核到内核。
    # sp -> kernel stack, sscratch -> user stack
    # restore sstatus/sepc/user_stack_sp
    ld t0, 32*8(sp) # t0 <- sstatus
    ld t1, 33*8(sp) # t1 <- sepc
    ld t2, 2*8(sp)  # t2 <- sp (user stack)
    sd tp, 34*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2
    
    # get SPP 在第8位
    # 之后要恢复t0，所以这里要先判断，后恢复
    andi t0, t0, 0x100
    bnez t0, __kernel_to_kernel

    ld x1, 1*8(sp)
    # skip x2(sp)，已保存
    .set n, 3
    .rept 29
        LOAD_GP %n
        .set n, n+1
    .endr
    addi sp, sp, 35*8
    csrrw sp, sscratch, sp
    # 如果在设置用户程序初始的trap之后，SPP设置为了User，但是还没有赶在switch之前触发异常
    # 之后就发生了内核中断，此时在restore时，会进入user的restore，但是又是内核 -> 内核
    # TODO：可不可以提前判断这种情况，从而相当于有三种分支？
    # 在restore的时候提前判断。现在先按照maturin的写法，求稳。
    beqz sp, __idle
    sret

__kernel_to_kernel:
    # 内核中断时，恢复现场，不用恢复fld，不用交换寄存器
    ld x1, 1*8(sp)
    # skip x2(sp)，已保存
    .set n, 3
    .rept 29
        LOAD_GP %n
        .set n, n+1
    .endr
    addi sp, sp, 35
    sret

__user_to_kernel:
    csrrw sp, sscratch, sp
    j __trap_entry

__idle:
    csrrw sp, sscratch, sp
    sret