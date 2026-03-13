(module
  ;; Struct with various field types including packed
  (type $point (struct (field (mut f64)) (field (mut f64))))
  (type $packed (struct (field (mut i8)) (field (mut i16)) (field (mut i32))))

  ;; struct.new - create a new struct from stack values
  (func (export "new_point") (param f64 f64) (result (ref $point))
    local.get 0
    local.get 1
    struct.new $point
  )

  ;; struct.new_default - create a struct with default values
  (func (export "default_point") (result (ref $point))
    struct.new_default $point
  )

  ;; struct.get - read an unpacked field
  (func (export "get_x") (param (ref null $point)) (result f64)
    local.get 0
    struct.get $point 0
  )

  ;; struct.set - write a field
  (func (export "set_x") (param (ref null $point) f64)
    local.get 0
    local.get 1
    struct.set $point 0
  )

  ;; struct.get_s - read a packed field with sign extension
  (func (export "get_i8_s") (param (ref null $packed)) (result i32)
    local.get 0
    struct.get_s $packed 0
  )

  ;; struct.get_u - read a packed field with zero extension
  (func (export "get_i8_u") (param (ref null $packed)) (result i32)
    local.get 0
    struct.get_u $packed 0
  )

  ;; struct.get_s for i16
  (func (export "get_i16_s") (param (ref null $packed)) (result i32)
    local.get 0
    struct.get_s $packed 1
  )

  ;; struct.get_u for i16
  (func (export "get_i16_u") (param (ref null $packed)) (result i32)
    local.get 0
    struct.get_u $packed 1
  )

  ;; struct.set for packed field
  (func (export "set_i8") (param (ref null $packed) i32)
    local.get 0
    local.get 1
    struct.set $packed 0
  )

  ;; struct.get for the i32 field (unpacked)
  (func (export "get_i32") (param (ref null $packed)) (result i32)
    local.get 0
    struct.get $packed 2
  )
)

(; CHECK-ALL:
  (module
    (type $point (;0;) (struct (field (mut f64)) (field (mut f64))))
    (type $packed (;1;) (struct (field (mut i8)) (field (mut i16)) (field (mut i32))))
    (type (;2;) (func (param f64 f64) (result (ref 0))))
    (type (;3;) (func (result (ref 0))))
    (type (;4;) (func (param (ref null 0)) (result f64)))
    (type (;5;) (func (param (ref null 0) f64)))
    (type (;6;) (func (param (ref null 1)) (result i32)))
    (type (;7;) (func (param (ref null 1) i32)))
    (func (;0;) (type 2) (param f64 f64) (result (ref 0))
      local.get 0
      local.get 1
      struct.new $point
    )
    (func (;1;) (type 5) (param (ref null 0) f64)
      local.get 0
      local.get 1
      struct.set $point 0
    )
    (func (;2;) (type 7) (param (ref null 1) i32)
      local.get 0
      local.get 1
      struct.set $packed 0
    )
    (func (;3;) (type 4) (param (ref null 0)) (result f64)
      local.get 0
      struct.get $point 0
    )
    (func (;4;) (type 6) (param (ref null 1)) (result i32)
      local.get 0
      struct.get_s $packed 0
    )
    (func (;5;) (type 6) (param (ref null 1)) (result i32)
      local.get 0
      struct.get_u $packed 0
    )
    (func (;6;) (type 6) (param (ref null 1)) (result i32)
      local.get 0
      struct.get_s $packed 1
    )
    (func (;7;) (type 6) (param (ref null 1)) (result i32)
      local.get 0
      struct.get_u $packed 1
    )
    (func (;8;) (type 6) (param (ref null 1)) (result i32)
      local.get 0
      struct.get $packed 2
    )
    (func (;9;) (type 3) (result (ref 0))
      struct.new_default $point
    )
    (export "new_point" (func 0))
    (export "default_point" (func 9))
    (export "get_x" (func 3))
    (export "set_x" (func 1))
    (export "get_i8_s" (func 4))
    (export "get_i8_u" (func 5))
    (export "get_i16_s" (func 6))
    (export "get_i16_u" (func 7))
    (export "set_i8" (func 2))
    (export "get_i32" (func 8))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
