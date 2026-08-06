// The probe succeeds when FEAT_LSE is available and traps otherwise.
.arch armv8.1-a+lse

.text
.global _start
.type _start, %function
_start:
  adrp x2, lse_word
  add x2, x2, :lo12:lse_word
  mov x0, #0
  mov x1, #1
  casal x0, x1, [x2]
  mov x0, #0
  mov x8, #93
  svc #0
.size _start, .-_start

.data
.balign 8
lse_word:
  .quad 0
