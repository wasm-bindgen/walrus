(module
  (global (mut i32) (i32.const 0))
  (export "a" (global 0)))

(; CHECK-ALL:
  (module
    (global (;0;) (mut i32) i32.const 0)
    (export "a" (global 0))
;)
