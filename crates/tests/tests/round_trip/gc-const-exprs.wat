(module
  ;; Struct and array types for const expression tests
  (type $point (struct (field (mut f64)) (field (mut f64))))
  (type $iarr (array (mut i32)))

  ;; Imports needed for extern/any conversion tests
  (import "env" "ext" (global $ext externref))
  (import "env" "any" (global $any anyref))

  ;; struct.new_default in global initializer
  (global (export "default_point") (ref $point) (struct.new_default $point))

  ;; struct.new with explicit field values in global initializer
  (global (export "explicit_point") (ref $point)
    (struct.new $point (f64.const 1.0) (f64.const 2.0)))

  ;; array.new_default in global initializer
  (global (export "default_arr") (ref $iarr)
    (array.new_default $iarr (i32.const 10)))

  ;; array.new in global initializer
  (global (export "filled_arr") (ref $iarr)
    (array.new $iarr (i32.const 42) (i32.const 5)))

  ;; array.new_fixed in global initializer
  (global (export "fixed_arr") (ref $iarr)
    (array.new_fixed $iarr 3 (i32.const 1) (i32.const 2) (i32.const 3)))

  ;; ref.i31 in global initializer (already supported, verify it works in combination)
  (global (export "i31val") i31ref (ref.i31 (i32.const 99)))

  ;; any.convert_extern in global initializer
  (global (export "as_any") anyref (any.convert_extern (global.get $ext)))

  ;; extern.convert_any in global initializer
  (global (export "as_extern") externref (extern.convert_any (global.get $any)))

  ;; Dummy export to keep the module alive
  (func (export "read_point") (param (ref null $point)) (result f64)
    local.get 0
    struct.get $point 0
  )
)

(; CHECK-ALL:
  (module
    (type $point (;0;) (struct (field (mut f64)) (field (mut f64))))
    (type $iarr (;1;) (array (mut i32)))
    (type (;2;) (func (param (ref null 0)) (result f64)))
    (import "env" "ext" (global $ext (;0;) externref))
    (import "env" "any" (global $any (;1;) anyref))
    (func (;0;) (type 2) (param (ref null 0)) (result f64)
      local.get 0
      struct.get $point 0
    )
    (global (;2;) (ref 0) struct.new_default $point)
    (global (;3;) (ref 0) f64.const 0x1p+0 (;=1;) f64.const 0x1p+1 (;=2;) struct.new $point)
    (global (;4;) (ref 1) i32.const 10 array.new_default $iarr)
    (global (;5;) (ref 1) i32.const 42 i32.const 5 array.new $iarr)
    (global (;6;) (ref 1) i32.const 1 i32.const 2 i32.const 3 array.new_fixed $iarr 3)
    (global (;7;) i31ref i32.const 99 ref.i31)
    (global (;8;) anyref global.get $ext any.convert_extern)
    (global (;9;) externref global.get $any extern.convert_any)
    (export "default_point" (global 2))
    (export "explicit_point" (global 3))
    (export "default_arr" (global 4))
    (export "filled_arr" (global 5))
    (export "fixed_arr" (global 6))
    (export "i31val" (global 7))
    (export "as_any" (global 8))
    (export "as_extern" (global 9))
    (export "read_point" (func 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
