(module
  ;; Base struct type (non-final to allow subtyping)
  (type $base (sub (struct (field i32))))

  ;; Derived struct type extending $base
  (type $derived (sub $base (struct (field i32) (field f64))))

  ;; Final derived type (cannot be further subtyped)
  (type $leaf (sub final $derived (struct (field i32) (field f64) (field i64))))

  ;; Base function type (non-final)
  (type $handler (sub (func (param i32) (result i32))))

  ;; Derived function type (covariant results, contravariant params)
  ;; Note: function subtyping requires exact match in current wasm GC spec
  ;; so we just test sub declarations

  ;; Function that uses these types to keep them alive
  (func (export "use_types") (param (ref null $base)) (param (ref null $derived)) (param (ref null $leaf)) (param (ref null $handler)) (result i32)
    i32.const 0
  )
)

(; CHECK-ALL:
  (module
    (type $base (;0;) (sub (struct (field i32))))
    (type $derived (;1;) (sub $base (struct (field i32) (field f64))))
    (type $leaf (;2;) (sub final $derived (struct (field i32) (field f64) (field i64))))
    (type $handler (;3;) (sub (func (param i32) (result i32))))
    (type (;4;) (func (param (ref null 0) (ref null 1) (ref null 2) (ref null 3)) (result i32)))
    (func (;0;) (type 4) (param (ref null 0) (ref null 1) (ref null 2) (ref null 3)) (result i32)
      i32.const 0
    )
    (export "use_types" (func 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
