(module
  (func (export "multiBlock") (param i64 i64) (result i64 i64 i64)
    (local.get 1)
    (local.get 0)
    (block (param i64 i64) (result i64 i64 i64)
      (i64.const 1234))))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i64 i64) (result i64 i64 i64)))
    (func (;0;) (type 0) (param i64 i64) (result i64 i64 i64)
      local.get 1
      local.get 0
      block (type 0) (param i64 i64) (result i64 i64 i64) ;; label = @1
        i64.const 1234
      end
    )
    (export "multiBlock" (func 0))
;)
