(module
  (type $byte_array (array (mut i8)))
  (type $funcref_array (array (mut funcref)))

  ;; Passive data segment for array.new_data / array.init_data
  (data $bytes "\01\02\03\04\05\06\07\08")

  ;; A helper function for element segment
  (func $helper (result i32)
    i32.const 42
  )

  ;; Passive element segment for array.new_elem / array.init_elem
  (elem $funcs func $helper $helper)

  ;; array.new_data - create array from a data segment
  (func (export "new_from_data") (result (ref $byte_array))
    i32.const 0  ;; offset in data segment
    i32.const 4  ;; length
    array.new_data $byte_array $bytes
  )

  ;; array.new_elem - create array from an element segment
  (func (export "new_from_elem") (result (ref $funcref_array))
    i32.const 0  ;; offset in element segment
    i32.const 2  ;; length
    array.new_elem $funcref_array $funcs
  )

  ;; array.init_data - initialize array elements from data segment
  (func (export "init_from_data") (param (ref null $byte_array))
    local.get 0
    i32.const 0  ;; dest offset in array
    i32.const 2  ;; source offset in data segment
    i32.const 4  ;; length
    array.init_data $byte_array $bytes
  )

  ;; array.init_elem - initialize array elements from element segment
  (func (export "init_from_elem") (param (ref null $funcref_array))
    local.get 0
    i32.const 0  ;; dest offset in array
    i32.const 0  ;; source offset in element segment
    i32.const 1  ;; length
    array.init_elem $funcref_array $funcs
  )
)

(; CHECK-ALL:
  (module
    (type $byte_array (;0;) (array (mut i8)))
    (type $funcref_array (;1;) (array (mut funcref)))
    (type (;2;) (func (result i32)))
    (type (;3;) (func (result (ref 0))))
    (type (;4;) (func (result (ref 1))))
    (type (;5;) (func (param (ref null 0))))
    (type (;6;) (func (param (ref null 1))))
    (func (;0;) (type 5) (param (ref null 0))
      local.get 0
      i32.const 0
      i32.const 2
      i32.const 4
      array.init_data $byte_array $bytes
    )
    (func (;1;) (type 6) (param (ref null 1))
      local.get 0
      i32.const 0
      i32.const 0
      i32.const 1
      array.init_elem $funcref_array $funcs
    )
    (func (;2;) (type 3) (result (ref 0))
      i32.const 0
      i32.const 4
      array.new_data $byte_array $bytes
    )
    (func (;3;) (type 4) (result (ref 1))
      i32.const 0
      i32.const 2
      array.new_elem $funcref_array $funcs
    )
    (func $helper (;4;) (type 2) (result i32)
      i32.const 42
    )
    (export "new_from_data" (func 2))
    (export "new_from_elem" (func 3))
    (export "init_from_data" (func 0))
    (export "init_from_elem" (func 1))
    (elem $funcs (;0;) func $helper $helper)
    (data $bytes (;0;) "\01\02\03\04\05\06\07\08")
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
