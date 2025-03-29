;; Do not gc used globals.

(module
  (global $used i32 (i32.const 666))
  (export "g" (global $used)))

(; CHECK-ALL:
  (module
    (global $used (;0;) i32 i32.const 666)
    (export "g" (global $used))
;)