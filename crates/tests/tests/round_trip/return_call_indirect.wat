(module
  (type (func (result i32)))
  (table 1 funcref)
  (func (export "a") (param i32) (result i32)
    local.get 0
    return_call_indirect (type 0)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (type (;1;) (func (param i32) (result i32)))
    (func (;0;) (type 1) (param i32) (result i32)
      local.get 0
      return_call_indirect (type 0)
    )
    (table (;0;) 1 funcref)
    (export "a" (func 0))
;)
