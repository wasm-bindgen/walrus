;; Round-trip test for call_ref and return_call_ref with a concrete function type.
;; Verifies the function type referenced by call_ref survives GC and encodes correctly.

(module
  (type $fn_ty (func (param i32) (result i32)))

  ;; Double via a direct call
  (func $double (type $fn_ty) (param i32) (result i32)
    local.get 0
    local.get 0
    i32.add
  )

  ;; Apply: takes a funcref and an i32, calls via call_ref
  (func (export "apply") (param (ref null $fn_ty) i32) (result i32)
    local.get 1
    local.get 0
    call_ref $fn_ty
  )

  ;; Tail-call version
  (func (export "apply_tail") (param (ref null $fn_ty) i32) (result i32)
    local.get 1
    local.get 0
    return_call_ref $fn_ty
  )

  (export "double" (func $double))
)

;; CHECK: (type $fn_ty (;0;) (func (param i32) (result i32)))
;; CHECK: call_ref $fn_ty
;; CHECK: return_call_ref $fn_ty
;; CHECK: (export "apply" (func 1))
;; CHECK: (export "apply_tail" (func 2))
;; CHECK: (export "double" (func $double))
