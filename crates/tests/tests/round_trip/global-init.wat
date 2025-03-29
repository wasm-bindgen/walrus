(module
  (import "x" "y" (global i32))
  (global i32 (global.get 0))
  (export "x" (global 1)))

(; CHECK-ALL:
  (module
    (import "x" "y" (global (;0;) i32))
    (global (;1;) i32 global.get 0)
    (export "x" (global 1))
;)
