(module
  (func (param i32 i32) (result i32)
    local.get 1
    local.get 0
    i32.add)
  (export "foo" (func 0)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32 i32) (result i32)))
    (func (;0;) (type 0) (param i32 i32) (result i32)
      local.get 1
      local.get 0
      i32.add
    )
    (export "foo" (func 0))
;)
