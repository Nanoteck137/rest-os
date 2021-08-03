//! Module to handle CPU interrupts

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct InterruptFrame {
	rip:    usize,
	cs:     usize,
	rflags: usize,
	rsp:    usize,
	ss:     usize,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct Regs {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9:  u64,
    r8:  u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
}

pub(super) fn initialize() {

}

#[no_mangle]
unsafe extern fn interrupt_handler(number: u8,
                                   frame: &mut InterruptFrame,
                                   error: u64,
                                   regs: &mut Regs)
{

}
const INT_HANDLERS: [unsafe extern fn(); 10] = [
    vec_interrupt_0,  vec_interrupt_1,  vec_interrupt_2,
    vec_interrupt_3,  vec_interrupt_4,  vec_interrupt_5,
    vec_interrupt_6,  vec_interrupt_7,  vec_interrupt_8,
    vec_interrupt_9
];

extern {
	fn vec_interrupt_0();
	fn vec_interrupt_1();
	fn vec_interrupt_2();
	fn vec_interrupt_3();
	fn vec_interrupt_4();
	fn vec_interrupt_5();
	fn vec_interrupt_6();
	fn vec_interrupt_7();
	fn vec_interrupt_8();
	fn vec_interrupt_9();
}

global_asm!(r#"
.extern interrupt_handler
enter_rust:
	push rax
	push rbx
	push qword ptr [r15 + 0x10]
	push qword ptr [r15 + 0x08]
	push rsi
	push rdi
	push rbp
	push qword ptr [r15 + 0x00]
	push r9
	push r10
	push r11
	push r12
	push r13
	push r14
	push qword ptr [r15 + 0x18]

    // Save the current stack pointer for the 4th argument
    mov  r9, rsp
    // Save the stack, allocate register homing space, and align the stack
    mov  rbp, rsp
    sub  rsp, 0x20
    and  rsp, ~0xf
	// Call the rust interrupt handler
	call interrupt_handler

    // Restore the stack
    mov rsp, rbp
	pop qword ptr [r15 + 0x18]
	pop r14
	pop r13
	pop r12
	pop r11
	pop r10
	pop r9
	pop qword ptr [r15 + 0x00]
	pop rbp
	pop rdi
	pop rsi
	pop qword ptr [r15 + 0x08]
	pop qword ptr [r15 + 0x10]
	pop rbx
	pop rax

	ret

.macro define_int_handler int_id, has_error_code
.global vec_interrupt_\int_id
vec_interrupt_\int_id:
    push r15
	push rcx
	push rdx
	push r8
    // Save off our "special" frame registers
    mov r15, rsp
.if \has_error_code
	mov  ecx, \int_id
	lea  rdx, [rsp+0x28]
	mov  r8,  [rsp+0x20]

	// 16-byte align the stack
	sub rsp, 8
.else
	mov ecx, \int_id
	lea rdx, [rsp+0x20]
	mov r8,  0
.endif
	call enter_rust

.if \has_error_code
	// Remove alignment from before
	add rsp, 8
.endif
	pop r8
	pop rdx
	pop rcx
    pop r15
.if \has_error_code
	// 'pop' off the error code
	add rsp, 8
.endif
	iretq
.endm

define_int_handler 0, 0
define_int_handler 1, 0
define_int_handler 2, 0
define_int_handler 3, 0
define_int_handler 4, 0
define_int_handler 5, 0
define_int_handler 6, 0
define_int_handler 7, 0
define_int_handler 8, 1
define_int_handler 9, 0
"#);

