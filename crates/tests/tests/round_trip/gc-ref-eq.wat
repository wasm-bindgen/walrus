(module
  (type $point (struct (field i32) (field i32)))

  ;; ref.eq - compare two eqref values
  (func (export "eq_null_null") (result i32)
    ref.null eq
    ref.null eq
    ref.eq
  )

  (func (export "eq_same") (param (ref null $point)) (result i32)
    local.get 0
    local.get 0
    ref.eq
  )

  (func (export "eq_different") (param (ref null $point)) (param (ref null $point)) (result i32)
    local.get 0
    local.get 1
    ref.eq
  )
)

(; CHECK-ALL:
  (module
    (type $point (;0;) (struct (field i32) (field i32)))
    (type (;1;) (func (result i32)))
    (type (;2;) (func (param (ref null $point)) (result i32)))
    (type (;3;) (func (param (ref null $point) (ref null $point)) (result i32)))
    (export "eq_null_null" (func 0))
    (export "eq_same" (func 1))
    (export "eq_different" (func 2))
    (func (;0;) (type 1) (result i32)
      ref.null eq
      ref.null eq
      ref.eq
    )
    (func (;1;) (type 2) (param (ref null $point)) (result i32)
      local.get 0
      local.get 0
      ref.eq
    )
    (func (;2;) (type 3) (param (ref null $point) (ref null $point)) (result i32)
      local.get 0
      local.get 1
      ref.eq
    )
;)
