# Rutinas de apoyo para MARS. runtime.rs selecciona los bloques BEGIN/END
# que necesita el programa y resuelve sus dependencias.

# BEGIN rt_print_int
rt_print_int:
    li $v0, 1
    syscall
    li $a0, 10
    li $v0, 11
    syscall
    jr $ra
# END rt_print_int

# BEGIN rt_print_str
rt_print_str:
    li $v0, 4
    syscall
    li $a0, 10
    li $v0, 11
    syscall
    jr $ra
# END rt_print_str

# BEGIN rt_print_bool
rt_print_bool:
    beqz $a0, rt_print_bool_false
    la $a0, rt_true
    j rt_print_bool_value
rt_print_bool_false:
    la $a0, rt_false
rt_print_bool_value:
    li $v0, 4
    syscall
    li $a0, 10
    li $v0, 11
    syscall
    jr $ra
# END rt_print_bool

# BEGIN rt_print_ref
rt_print_ref:
    beqz $a0, rt_print_ref_null
    li $v0, 1
    syscall
    j rt_print_ref_newline
rt_print_ref_null:
    la $a0, rt_null
    li $v0, 4
    syscall
rt_print_ref_newline:
    li $a0, 10
    li $v0, 11
    syscall
    jr $ra
# END rt_print_ref

# BEGIN rt_alloc
rt_alloc:
    li $v0, 9
    syscall
    jr $ra
# END rt_alloc

# BEGIN rt_concat
rt_concat:
    addiu $sp, $sp, -16
    sw $a0, 0($sp)
    sw $a1, 4($sp)
    sw $ra, 12($sp)
    move $t8, $a0
    li $v1, 0
rt_concat_len_left:
    lbu $t9, 0($t8)
    beqz $t9, rt_concat_len_left_done
    addiu $v1, $v1, 1
    addiu $t8, $t8, 1
    j rt_concat_len_left
rt_concat_len_left_done:
    sw $v1, 8($sp)
    lw $t8, 4($sp)
    li $v1, 0
rt_concat_len_right:
    lbu $t9, 0($t8)
    beqz $t9, rt_concat_len_right_done
    addiu $v1, $v1, 1
    addiu $t8, $t8, 1
    j rt_concat_len_right
rt_concat_len_right_done:
    lw $t8, 8($sp)
    addu $a0, $t8, $v1
    addiu $a0, $a0, 1
    jal rt_alloc
    move $t8, $v0
    lw $t9, 0($sp)
rt_concat_copy_left:
    lbu $v1, 0($t9)
    beqz $v1, rt_concat_copy_right_start
    sb $v1, 0($t8)
    addiu $t9, $t9, 1
    addiu $t8, $t8, 1
    j rt_concat_copy_left
rt_concat_copy_right_start:
    lw $t9, 4($sp)
rt_concat_copy_right:
    lbu $v1, 0($t9)
    sb $v1, 0($t8)
    beqz $v1, rt_concat_done
    addiu $t9, $t9, 1
    addiu $t8, $t8, 1
    j rt_concat_copy_right
rt_concat_done:
    lw $ra, 12($sp)
    addiu $sp, $sp, 16
    jr $ra
# END rt_concat

# BEGIN rt_streq
rt_streq:
    move $t8, $a0
    move $t9, $a1
rt_streq_loop:
    lbu $v0, 0($t8)
    lbu $v1, 0($t9)
    bne $v0, $v1, rt_streq_false
    beqz $v0, rt_streq_true
    addiu $t8, $t8, 1
    addiu $t9, $t9, 1
    j rt_streq_loop
rt_streq_false:
    li $v0, 0
    jr $ra
rt_streq_true:
    li $v0, 1
    jr $ra
# END rt_streq

# BEGIN rt_check_bounds
rt_check_bounds:
    bltz $a1, rt_check_bounds_error
    lw $t8, 0($a0)
    bge $a1, $t8, rt_check_bounds_error
    jr $ra
rt_check_bounds_error:
    la $a0, rt_msg_index
    j rt_error
# END rt_check_bounds

# BEGIN rt_err_div0
rt_err_div0:
    la $a0, rt_msg_div0
    j rt_error
# END rt_err_div0

# BEGIN rt_error
rt_error:
    lw $t8, rt_try_top
    beqz $t8, rt_error_fatal
    lw $t9, 0($t8)
    sw $t9, rt_try_top
    sw $a0, rt_exception
    lw $fp, 8($t8)
    lw $s0, 12($t8)
    lw $s1, 16($t8)
    lw $s2, 20($t8)
    lw $s3, 24($t8)
    lw $s4, 28($t8)
    lw $s5, 32($t8)
    lw $s6, 36($t8)
    lw $s7, 40($t8)
    lw $t9, 4($t8)
    addiu $sp, $t8, 48
    jr $t9
rt_error_fatal:
    move $t8, $a0
    la $a0, rt_error_prefix
    li $v0, 4
    syscall
    move $a0, $t8
    li $v0, 4
    syscall
    li $a0, 10
    li $v0, 11
    syscall
    li $a0, 1
    li $v0, 17
    syscall
# END rt_error
