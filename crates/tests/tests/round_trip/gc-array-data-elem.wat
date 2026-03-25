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

;; CHECK: (module
;; NEXT: (type $byte_array (;0;) (array (mut i8)))
;; NEXT: (type $funcref_array (;1;) (array (mut funcref)))
;; NEXT: (type (;2;) (func (result i32)))
;; NEXT: (type (;3;) (func (result (ref 0))))
;; NEXT: (type (;4;) (func (result (ref 1))))
;; NEXT: (type (;5;) (func (param (ref null 0))))
;; NEXT: (type (;6;) (func (param (ref null 1))))
;; NEXT: (func (;0;) (type 5) (param (ref null 0))
;; NEXT: local.get 0
;; NEXT: i32.const 0
;; NEXT: i32.const 2
;; NEXT: i32.const 4
;; NEXT: array.init_data $byte_array $bytes
;; NEXT: )
;; NEXT: (func (;1;) (type 6) (param (ref null 1))
;; NEXT: local.get 0
;; NEXT: i32.const 0
;; NEXT: i32.const 0
;; NEXT: i32.const 1
;; NEXT: array.init_elem $funcref_array $funcs
;; NEXT: )
;; NEXT: (func (;2;) (type 3) (result (ref 0))
;; NEXT: i32.const 0
;; NEXT: i32.const 4
;; NEXT: array.new_data $byte_array $bytes
;; NEXT: )
;; NEXT: (func (;3;) (type 4) (result (ref 1))
;; NEXT: i32.const 0
;; NEXT: i32.const 2
;; NEXT: array.new_elem $funcref_array $funcs
;; NEXT: )
;; NEXT: (func $helper (;4;) (type 2) (result i32)
;; NEXT: i32.const 42
;; NEXT: )
;; NEXT: (export "new_from_data" (func 2))
;; NEXT: (export "new_from_elem" (func 3))
;; NEXT: (export "init_from_data" (func 0))
;; NEXT: (export "init_from_elem" (func 1))
;; NEXT: (elem $funcs (;0;) func $helper $helper)
;; NEXT: (data $bytes (;0;) "\01\02\03\04\05\06\07\08")
