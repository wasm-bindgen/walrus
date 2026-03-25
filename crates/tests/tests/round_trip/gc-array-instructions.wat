(module
  (type $i32_array (array (mut i32)))
  (type $byte_array (array (mut i8)))
  (type $f64_array (array (mut f64)))

  ;; array.new - create array filled with a value
  (func (export "new_i32_array") (param i32 i32) (result (ref $i32_array))
    local.get 0
    local.get 1
    array.new $i32_array
  )

  ;; array.new_default - create array with default values
  (func (export "new_default") (param i32) (result (ref $i32_array))
    local.get 0
    array.new_default $i32_array
  )

  ;; array.new_fixed - create array from stack values
  (func (export "new_fixed") (result (ref $i32_array))
    i32.const 10
    i32.const 20
    i32.const 30
    array.new_fixed $i32_array 3
  )

  ;; array.get - read an element
  (func (export "get_i32") (param (ref null $i32_array) i32) (result i32)
    local.get 0
    local.get 1
    array.get $i32_array
  )

  ;; array.set - write an element
  (func (export "set_i32") (param (ref null $i32_array) i32 i32)
    local.get 0
    local.get 1
    local.get 2
    array.set $i32_array
  )

  ;; array.len - get the length
  (func (export "len") (param (ref null $i32_array)) (result i32)
    local.get 0
    array.len
  )

  ;; array.get_s - read packed element with sign extension
  (func (export "get_byte_s") (param (ref null $byte_array) i32) (result i32)
    local.get 0
    local.get 1
    array.get_s $byte_array
  )

  ;; array.get_u - read packed element with zero extension
  (func (export "get_byte_u") (param (ref null $byte_array) i32) (result i32)
    local.get 0
    local.get 1
    array.get_u $byte_array
  )

  ;; array.set for packed array
  (func (export "set_byte") (param (ref null $byte_array) i32 i32)
    local.get 0
    local.get 1
    local.get 2
    array.set $byte_array
  )

  ;; array.fill - fill a range with a value
  (func (export "fill") (param (ref null $i32_array) i32 i32 i32)
    local.get 0
    local.get 1
    local.get 2
    local.get 3
    array.fill $i32_array
  )

  ;; array.copy - copy between arrays
  (func (export "copy") (param (ref null $i32_array) i32 (ref null $i32_array) i32 i32)
    local.get 0
    local.get 1
    local.get 2
    local.get 3
    local.get 4
    array.copy $i32_array $i32_array
  )
)

;; CHECK: (module
;; NEXT: (type $i32_array (;0;) (array (mut i32)))
;; NEXT: (type $byte_array (;1;) (array (mut i8)))
;; NEXT: (type (;2;) (func (param i32 i32) (result (ref 0))))
;; NEXT: (type (;3;) (func (param i32) (result (ref 0))))
;; NEXT: (type (;4;) (func (result (ref 0))))
;; NEXT: (type (;5;) (func (param (ref null 0) i32) (result i32)))
;; NEXT: (type (;6;) (func (param (ref null 0) i32 i32)))
;; NEXT: (type (;7;) (func (param (ref null 0)) (result i32)))
;; NEXT: (type (;8;) (func (param (ref null 1) i32) (result i32)))
;; NEXT: (type (;9;) (func (param (ref null 1) i32 i32)))
;; NEXT: (type (;10;) (func (param (ref null 0) i32 i32 i32)))
;; NEXT: (type (;11;) (func (param (ref null 0) i32 (ref null 0) i32 i32)))
;; NEXT: (func (;0;) (type 11) (param (ref null 0) i32 (ref null 0) i32 i32)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: local.get 2
;; NEXT: local.get 3
;; NEXT: local.get 4
;; NEXT: array.copy $i32_array $i32_array
;; NEXT: )
;; NEXT: (func (;1;) (type 10) (param (ref null 0) i32 i32 i32)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: local.get 2
;; NEXT: local.get 3
;; NEXT: array.fill $i32_array
;; NEXT: )
;; NEXT: (func (;2;) (type 4) (result (ref 0))
;; NEXT: i32.const 10
;; NEXT: i32.const 20
;; NEXT: i32.const 30
;; NEXT: array.new_fixed $i32_array 3
;; NEXT: )
;; NEXT: (func (;3;) (type 6) (param (ref null 0) i32 i32)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: local.get 2
;; NEXT: array.set $i32_array
;; NEXT: )
;; NEXT: (func (;4;) (type 9) (param (ref null 1) i32 i32)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: local.get 2
;; NEXT: array.set $byte_array
;; NEXT: )
;; NEXT: (func (;5;) (type 2) (param i32 i32) (result (ref 0))
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: array.new $i32_array
;; NEXT: )
;; NEXT: (func (;6;) (type 5) (param (ref null 0) i32) (result i32)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: array.get $i32_array
;; NEXT: )
;; NEXT: (func (;7;) (type 8) (param (ref null 1) i32) (result i32)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: array.get_s $byte_array
;; NEXT: )
;; NEXT: (func (;8;) (type 8) (param (ref null 1) i32) (result i32)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: array.get_u $byte_array
;; NEXT: )
;; NEXT: (func (;9;) (type 3) (param i32) (result (ref 0))
;; NEXT: local.get 0
;; NEXT: array.new_default $i32_array
;; NEXT: )
;; NEXT: (func (;10;) (type 7) (param (ref null 0)) (result i32)
;; NEXT: local.get 0
;; NEXT: array.len
;; NEXT: )
;; NEXT: (export "new_i32_array" (func 5))
;; NEXT: (export "new_default" (func 9))
;; NEXT: (export "new_fixed" (func 2))
;; NEXT: (export "get_i32" (func 6))
;; NEXT: (export "set_i32" (func 3))
;; NEXT: (export "len" (func 10))
;; NEXT: (export "get_byte_s" (func 7))
;; NEXT: (export "get_byte_u" (func 8))
;; NEXT: (export "set_byte" (func 4))
;; NEXT: (export "fill" (func 1))
;; NEXT: (export "copy" (func 0))
