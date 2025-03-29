(module
  (func (export "multiLoop") (param i32 i64 i64) (result i64 i64)
    (local.get 2)
    (local.get 1)
    (local.get 0)
    (if (param i64 i64) (result i64 i64)
      (then return)
      (else
        (drop)
        (drop)
        (i64.const 0)
        (i64.const 0)))))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32 i64 i64) (result i64 i64)))
    (type (;1;) (func (param i64 i64) (result i64 i64)))
    (func (;0;) (type 0) (param i32 i64 i64) (result i64 i64)
      local.get 2
      local.get 1
      local.get 0
      if (type 1) (param i64 i64) (result i64 i64) ;; label = @1
        return
      else
        drop
        drop
        i64.const 0
        i64.const 0
      end
    )
    (export "multiLoop" (func 0))
;)
