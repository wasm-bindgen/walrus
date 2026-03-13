(module
  ;; Simple array of i32
  (type $int_array (array i32))

  ;; Mutable array of f64
  (type $float_array (array (mut f64)))

  ;; Packed array of i8 (byte array)
  (type $byte_array (array (mut i8)))

  ;; Packed array of i16
  (type $short_array (array i16))

  ;; Array of references
  (type $ref_array (array (mut (ref null $int_array))))

  ;; Function that uses array types to keep them alive
  (func (export "use_types") (param (ref null $int_array)) (param (ref null $float_array)) (param (ref null $byte_array)) (param (ref null $short_array)) (param (ref null $ref_array)) (result i32)
    i32.const 0
  )
)

(; CHECK-ALL:
  (module
    (type $int_array (;0;) (array i32))
    (type $float_array (;1;) (array (mut f64)))
    (type $byte_array (;2;) (array (mut i8)))
    (type $short_array (;3;) (array i16))
    (type $ref_array (;4;) (array (mut (ref null 0))))
    (type (;5;) (func (param (ref null 0) (ref null 1) (ref null 2) (ref null 3) (ref null 4)) (result i32)))
    (func (;0;) (type 5) (param (ref null 0) (ref null 1) (ref null 2) (ref null 3) (ref null 4)) (result i32)
      i32.const 0
    )
    (export "use_types" (func 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
