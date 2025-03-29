;; The function entry's `InstrSeq` implicitly has type `[] -> [i64 i64]` but
;; that type should *NOT* appear in the emitted Type section.

(module
  (func (export "multiLoop") (param i64 i64) (result i64 i64)
    (local.get 1)
    (local.get 0)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i64 i64) (result i64 i64)))
    (func (;0;) (type 0) (param i64 i64) (result i64 i64)
      local.get 1
      local.get 0
    )
    (export "multiLoop" (func 0))
;)