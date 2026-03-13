(module
  ;; Simple struct type
  (type $point (struct (field f64) (field f64)))

  ;; Struct with named fields and mutability
  (type $mutable_pair (struct (field (mut i32)) (field (mut i64))))

  ;; Struct with packed fields
  (type $packed (struct (field i8) (field i16) (field i32)))

  ;; Struct with ref fields
  (type $with_ref (struct (field (ref null $point)) (field i32)))

  ;; Function that uses struct types to keep them alive
  (func (export "use_types") (param (ref null $point)) (param (ref null $mutable_pair)) (param (ref null $packed)) (param (ref null $with_ref)) (result i32)
    i32.const 0
  )
)

(; CHECK-ALL:
  (module
    (type $point (;0;) (struct (field f64) (field f64)))
    (type $mutable_pair (;1;) (struct (field (mut i32)) (field (mut i64))))
    (type $packed (;2;) (struct (field i8) (field i16) (field i32)))
    (type $with_ref (;3;) (struct (field (ref null 0)) (field i32)))
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
