(module
  (func (export "multiLoop") (param i64 i64) (result i64 i64)
    (local.get 1)
    (local.get 0)
    (loop (param i64 i64) (result i64 i64)
       return)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i64 i64) (result i64 i64)))
    (func (;0;) (type 0) (param i64 i64) (result i64 i64)
      local.get 1
      local.get 0
      loop (type 0) (param i64 i64) (result i64 i64) ;; label = @1
        return
      end
    )
    (export "multiLoop" (func 0))
;)
