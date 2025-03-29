(module
  (type (func))
  (table 1 funcref)
  (table 1 externref)
  (func (export "a") (param i32) (result externref)
    local.get 0
    call_indirect (type 0)
    local.get 0
    table.get 1))

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (type (;1;) (func (param i32) (result externref)))
    (func (;0;) (type 1) (param i32) (result externref)
      local.get 0
      call_indirect (type 0)
      local.get 0
      table.get 1
    )
    (table (;0;) 1 funcref)
    (table (;1;) 1 externref)
    (export "a" (func 0))
;)