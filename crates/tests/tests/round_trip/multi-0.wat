(module
  (func (export "i64.dup") (param i64) (result i64 i64)
    (local.get 0) (local.get 0)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i64) (result i64 i64)))
    (func (;0;) (type 0) (param i64) (result i64 i64)
      local.get 0
      local.get 0
    )
    (export "i64.dup" (func 0))
;)
