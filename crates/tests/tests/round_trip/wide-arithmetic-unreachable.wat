;; Wide arithmetic instructions in unreachable code paths.

(module
  ;; wide arithmetic after unreachable - should survive round trip
  (func (export "add128_unreachable") (result i64 i64)
    unreachable
    i64.const 1
    i64.const 0
    i64.const 2
    i64.const 0
    i64.add128
  )

  ;; wide arithmetic used normally, then unreachable after
  (func (export "sub128_then_unreachable") (param i64 i64 i64 i64) (result i64 i64)
    local.get 0
    local.get 1
    local.get 2
    local.get 3
    i64.sub128
    unreachable
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i64 i64)))
    (type (;1;) (func (param i64 i64 i64 i64) (result i64 i64)))
    (export "add128_unreachable" (func 1))
    (export "sub128_then_unreachable" (func 0))
    (func (;0;) (type 1) (param i64 i64 i64 i64) (result i64 i64)
      local.get 0
      local.get 1
      local.get 2
      local.get 3
      i64.sub128
      unreachable
    )
    (func (;1;) (type 0) (result i64 i64)
      unreachable
    )
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
