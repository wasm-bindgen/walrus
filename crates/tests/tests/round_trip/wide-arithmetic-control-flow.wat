;; Wide arithmetic instructions inside control flow structures (block, loop, if/else).

(module
  ;; wide arithmetic inside a block
  (func (export "add128_in_block") (param i64 i64 i64 i64) (result i64 i64)
    block (result i64 i64)
      local.get 0
      local.get 1
      local.get 2
      local.get 3
      i64.add128
    end
  )

  ;; wide arithmetic inside a loop
  (func (export "sub128_in_loop") (param i64 i64 i64 i64) (result i64 i64)
    loop (result i64 i64)
      local.get 0
      local.get 1
      local.get 2
      local.get 3
      i64.sub128
    end
  )

  ;; wide arithmetic inside if/else branches
  (func (export "mul_wide_in_if") (param i32 i64 i64) (result i64 i64)
    local.get 0
    if (result i64 i64)
      local.get 1
      local.get 2
      i64.mul_wide_s
    else
      local.get 1
      local.get 2
      i64.mul_wide_u
    end
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i64 i64)))
    (type (;1;) (func (param i32 i64 i64) (result i64 i64)))
    (type (;2;) (func (param i64 i64 i64 i64) (result i64 i64)))
    (export "add128_in_block" (func 1))
    (export "sub128_in_loop" (func 2))
    (export "mul_wide_in_if" (func 0))
    (func (;0;) (type 1) (param i32 i64 i64) (result i64 i64)
      local.get 0
      if (type 0) (result i64 i64) ;; label = @1
        local.get 1
        local.get 2
        i64.mul_wide_s
      else
        local.get 1
        local.get 2
        i64.mul_wide_u
      end
    )
    (func (;1;) (type 2) (param i64 i64 i64 i64) (result i64 i64)
      block (type 0) (result i64 i64) ;; label = @1
        local.get 0
        local.get 1
        local.get 2
        local.get 3
        i64.add128
      end
    )
    (func (;2;) (type 2) (param i64 i64 i64 i64) (result i64 i64)
      loop (type 0) (result i64 i64) ;; label = @1
        local.get 0
        local.get 1
        local.get 2
        local.get 3
        i64.sub128
      end
    )
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
