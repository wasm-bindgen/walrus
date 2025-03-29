(module
  ;; Iterative factorial without locals.
  (func $pick0 (param i64) (result i64 i64)
    (local.get 0) (local.get 0)
  )
  (func $pick1 (param i64 i64) (result i64 i64 i64)
    (local.get 0) (local.get 1) (local.get 0)
  )
  (func (export "fac-ssa") (param i64) (result i64)
    (i64.const 1) (local.get 0)
    (loop $l (param i64 i64) (result i64)
      (call $pick1) (call $pick1) (i64.mul)
      (call $pick1) (i64.const 1) (i64.sub)
      (call $pick0) (i64.const 0) (i64.gt_u)
      (br_if $l)
      (drop) (return)
    )
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i64) (result i64)))
    (type (;1;) (func (param i64) (result i64 i64)))
    (type (;2;) (func (param i64 i64) (result i64)))
    (type (;3;) (func (param i64 i64) (result i64 i64 i64)))
    (func (;0;) (type 0) (param i64) (result i64)
      i64.const 1
      local.get 0
      loop (type 2) (param i64 i64) (result i64) ;; label = @1
        call $pick1
        call $pick1
        i64.mul
        call $pick1
        i64.const 1
        i64.sub
        call $pick0
        i64.const 0
        i64.gt_u
        br_if 0 (;@1;)
        drop
        return
      end
    )
    (func $pick1 (;1;) (type 3) (param i64 i64) (result i64 i64 i64)
      local.get 0
      local.get 1
      local.get 0
    )
    (func $pick0 (;2;) (type 1) (param i64) (result i64 i64)
      local.get 0
      local.get 0
    )
    (export "fac-ssa" (func 0))
;)