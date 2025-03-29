(module
  (import "spectest" "print_i32_f32" (func $print_i32_f32 (param i32 f32)))

  (func (export "print32")
    (call $print_i32_f32
      (i32.const 1)
      (f32.const 42)
    )
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (type (;1;) (func (param i32 f32)))
    (import "spectest" "print_i32_f32" (func $print_i32_f32 (;0;) (type 1)))
    (func (;1;) (type 0)
      i32.const 1
      f32.const 0x1.5p+5 (;=42;)
      call $print_i32_f32
    )
    (export "print32" (func 1))
;)