(module
  (global i32 (i32.const 0))
  (func $foo
    (local i32 i32)
    local.get 0
    local.set 1)
  (export "foo" (func $foo))
  )

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (func $foo (;0;) (type 0)
      (local i32 i32)
      local.get 0
      local.set 1
    )
    (export "foo" (func $foo))
;)
