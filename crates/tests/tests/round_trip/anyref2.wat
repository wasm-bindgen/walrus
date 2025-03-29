(module
  (table 1 externref)
  (func (export "a") (param i32) (result externref)
    local.get 0
    table.get 0))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32) (result externref)))
    (func (;0;) (type 0) (param i32) (result externref)
      local.get 0
      table.get 0
    )
    (table (;0;) 1 externref)
    (export "a" (func 0))
;)