(module
  ;; i64.add128 - 128-bit addition
  (func (export "add128") (param i64 i64 i64 i64) (result i64 i64)
    local.get 0
    local.get 1
    local.get 2
    local.get 3
    i64.add128
  )

  ;; i64.sub128 - 128-bit subtraction
  (func (export "sub128") (param i64 i64 i64 i64) (result i64 i64)
    local.get 0
    local.get 1
    local.get 2
    local.get 3
    i64.sub128
  )

  ;; i64.mul_wide_s - signed widening multiplication
  (func (export "mul_wide_s") (param i64 i64) (result i64 i64)
    local.get 0
    local.get 1
    i64.mul_wide_s
  )

  ;; i64.mul_wide_u - unsigned widening multiplication
  (func (export "mul_wide_u") (param i64 i64) (result i64 i64)
    local.get 0
    local.get 1
    i64.mul_wide_u
  )

  ;; Overflow detection using i64.add128 with zero-extended operands
  (func (export "overflowing_add") (param i64 i64) (result i64 i64)
    local.get 0
    i64.const 0
    local.get 1
    i64.const 0
    i64.add128
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i64 i64) (result i64 i64)))
    (type (;1;) (func (param i64 i64 i64 i64) (result i64 i64)))
    (export "add128" (func 0))
    (export "sub128" (func 1))
    (export "mul_wide_s" (func 3))
    (export "mul_wide_u" (func 4))
    (export "overflowing_add" (func 2))
    (func (;0;) (type 1) (param i64 i64 i64 i64) (result i64 i64)
      local.get 0
      local.get 1
      local.get 2
      local.get 3
      i64.add128
    )
    (func (;1;) (type 1) (param i64 i64 i64 i64) (result i64 i64)
      local.get 0
      local.get 1
      local.get 2
      local.get 3
      i64.sub128
    )
    (func (;2;) (type 0) (param i64 i64) (result i64 i64)
      local.get 0
      i64.const 0
      local.get 1
      i64.const 0
      i64.add128
    )
    (func (;3;) (type 0) (param i64 i64) (result i64 i64)
      local.get 0
      local.get 1
      i64.mul_wide_s
    )
    (func (;4;) (type 0) (param i64 i64) (result i64 i64)
      local.get 0
      local.get 1
      i64.mul_wide_u
    )
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
