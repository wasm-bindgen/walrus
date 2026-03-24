;; GC pass should remove unused functions containing wide arithmetic instructions.

(module
  ;; This function is unused and should be GC'd
  (func $unused_add128 (param i64 i64 i64 i64) (result i64 i64)
    local.get 0
    local.get 1
    local.get 2
    local.get 3
    i64.add128
  )

  ;; This function is unused and should be GC'd
  (func $unused_mul_wide (param i64 i64) (result i64 i64)
    local.get 0
    local.get 1
    i64.mul_wide_u
  )

  ;; This function is exported and should be kept
  (func (export "kept_sub128") (param i64 i64 i64 i64) (result i64 i64)
    local.get 0
    local.get 1
    local.get 2
    local.get 3
    i64.sub128
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i64 i64 i64 i64) (result i64 i64)))
    (export "kept_sub128" (func 0))
    (func (;0;) (type 0) (param i64 i64 i64 i64) (result i64 i64)
      local.get 0
      local.get 1
      local.get 2
      local.get 3
      i64.sub128
    )
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
