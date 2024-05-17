.altmacro
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
.macro LOAD_AT n
    ld s\n, (\n+2)*8(a0)
.endm

    .section .text
    .globl __switch
__switch:
    # a0: current_task_cx_ptr  a1: next_task_cx_ptr
    # ret: 普通的任务，返回ra处，即 __restore; idle, 返回 __switch函数的后一条指令
    sd ra, 0(a0)
    sd sp, 8(a0)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n+1
    .endr
    
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n+1
    .endr
    ld sp, 8(a1)
    ld ra, 0(a1)
    ret

    .section .text
    .globl __switch_to_idle
__switch_to_idle:
# a0: idle_cx_ptr
    .set n, 0
    .rept 12
        LOAD_AT %n
        .set n, n+1
    .endr
    ld sp, 8(a0)
    ld ra, 0(a0)
    ret

