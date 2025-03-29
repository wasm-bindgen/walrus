(module
  (func (export "as-br-value") (param i32) (result i32)
    (block (result i32) (br 0 (local.get 0)))
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32) (result i32)))
    (func (;0;) (type 0) (param i32) (result i32)
      block (result i32) ;; label = @1
        local.get 0
        br 0 (;@1;)
      end
    )
    (export "as-br-value" (func 0))
;)
