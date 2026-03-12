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

(; CHECK-ALL:
  (module
    (type $i32_array (;0;) (array (mut i32)))
    (type $byte_array (;1;) (array (mut i8)))
    (type (;2;) (func (param i32 i32) (result (ref 0))))
    (type (;3;) (func (param i32) (result (ref 0))))
    (type (;4;) (func (result (ref 0))))
    (type (;5;) (func (param (ref null 0) i32) (result i32)))
    (type (;6;) (func (param (ref null 0) i32 i32)))
    (type (;7;) (func (param (ref null 0)) (result i32)))
    (type (;8;) (func (param (ref null 1) i32) (result i32)))
    (type (;9;) (func (param (ref null 1) i32 i32)))
    (type (;10;) (func (param (ref null 0) i32 i32 i32)))
    (type (;11;) (func (param (ref null 0) i32 (ref null 0) i32 i32)))
    (func (;0;) (type 11) (param (ref null 0) i32 (ref null 0) i32 i32)
      local.get 0
      local.get 1
      local.get 2
      local.get 3
      local.get 4
      array.copy $i32_array $i32_array
    )
    (func (;1;) (type 10) (param (ref null 0) i32 i32 i32)
      local.get 0
      local.get 1
      local.get 2
      local.get 3
      array.fill $i32_array
    )
    (func (;2;) (type 4) (result (ref 0))
      i32.const 10
      i32.const 20
      i32.const 30
      array.new_fixed $i32_array 3
    )
    (func (;3;) (type 6) (param (ref null 0) i32 i32)
      local.get 0
      local.get 1
      local.get 2
      array.set $i32_array
    )
    (func (;4;) (type 9) (param (ref null 1) i32 i32)
      local.get 0
      local.get 1
      local.get 2
      array.set $byte_array
    )
    (func (;5;) (type 2) (param i32 i32) (result (ref 0))
      local.get 0
      local.get 1
      array.new $i32_array
    )
    (func (;6;) (type 5) (param (ref null 0) i32) (result i32)
      local.get 0
      local.get 1
      array.get $i32_array
    )
    (func (;7;) (type 8) (param (ref null 1) i32) (result i32)
      local.get 0
      local.get 1
      array.get_s $byte_array
    )
    (func (;8;) (type 8) (param (ref null 1) i32) (result i32)
      local.get 0
      local.get 1
      array.get_u $byte_array
    )
    (func (;9;) (type 3) (param i32) (result (ref 0))
      local.get 0
      array.new_default $i32_array
    )
    (func (;10;) (type 7) (param (ref null 0)) (result i32)
      local.get 0
      array.len
    )
    (export "new_i32_array" (func 5))
    (export "new_default" (func 9))
    (export "new_fixed" (func 2))
    (export "get_i32" (func 6))
    (export "set_i32" (func 3))
    (export "len" (func 10))
    (export "get_byte_s" (func 7))
    (export "get_byte_u" (func 8))
    (export "set_byte" (func 4))
    (export "fill" (func 1))
    (export "copy" (func 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
