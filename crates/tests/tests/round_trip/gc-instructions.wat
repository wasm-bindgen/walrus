(module
  ;; ref.i31 - Create i31ref from i32
  
  (func (export "make_i31") (param i32) (result i31ref)
    local.get 0
    ref.i31
  )
  
  (func (export "roundtrip_pos") (result i32)
    i32.const 42
    ref.i31
    i31.get_u
  )
  
  (func (export "roundtrip_neg") (result i32)
    i32.const -1
    ref.i31
    i31.get_s
  )

  ;; i31.get_s - Extract signed i32 from i31ref
  
  (func (export "get_s_positive") (result i32)
    i32.const 100
    ref.i31
    i31.get_s
  )
  
  (func (export "get_s_max_positive") (result i32)
    i32.const 0x3FFFFFFF
    ref.i31
    i31.get_s
  )

  ;; i31.get_u - Extract unsigned i32 from i31ref
  
  (func (export "get_u_basic") (result i32)
    i32.const 100
    ref.i31
    i31.get_u
  )
  
  (func (export "get_u_max") (result i32)
    i32.const 0x7FFFFFFF
    ref.i31
    i31.get_u
  )

  ;; ref.test - Test if reference matches type
  
  (func (export "test_is_i31") (param anyref) (result i32)
    local.get 0
    ref.test (ref null i31)
  )
  
  (func (export "test_null_nullable") (result i32)
    ref.null any
    ref.test (ref null i31)
  )
  
  (func (export "test_null_nonnullable") (result i32)
    ref.null any
    ref.test (ref i31)
  )
  
  (func (export "test_i31_is_i31") (result i32)
    i32.const 42
    ref.i31
    ref.test (ref null i31)
  )

  ;; ref.cast - Cast reference to type
  
  (func (export "cast_i31_to_i31") (result i32)
    i32.const 42
    ref.i31
    ref.cast (ref i31)
    i31.get_u
  )
  
  (func (export "cast_null_nullable") (result i31ref)
    ref.null any
    ref.cast (ref null i31)
  )
  
  (func (export "cast_any_to_i31") (param anyref) (result i31ref)
    local.get 0
    ref.cast (ref null i31)
  )

  ;; br_on_cast - Branch if cast succeeds
  
  (func (export "br_on_cast_i31") (param anyref) (result i32)
    (block $is_i31 (result i31ref)
      local.get 0
      br_on_cast $is_i31 (ref null any) (ref null i31)
      drop
      i32.const -1
      return
    )
    i31.get_u
  )

  ;; br_on_cast_fail - Branch if cast fails
  
  (func (export "br_on_cast_fail_i31") (param anyref) (result i32)
    (block $not_i31 (result anyref)
      local.get 0
      br_on_cast_fail $not_i31 (ref null any) (ref null i31)
      i31.get_u
      return
    )
    drop
    i32.const -1
  )

  ;; ref.null with new heap types
  
  (func (export "null_any") (result anyref)
    ref.null any
  )
  
  (func (export "null_i31") (result i31ref)
    ref.null i31
  )
  
  (func (export "null_eq") (result eqref)
    ref.null eq
  )
  
  (func (export "null_any_is_null") (result i32)
    ref.null any
    ref.is_null
  )

  ;; any.convert_extern and extern.convert_any
  
  (table $t 10 anyref)
  
  (func (export "store_extern_as_any") (param i32 externref)
    local.get 0
    local.get 1
    any.convert_extern
    table.set $t
  )
  
  (func (export "load_any_as_extern") (param i32) (result externref)
    local.get 0
    table.get $t
    extern.convert_any
  )
  
  (func (export "roundtrip_extern") (param externref) (result externref)
    local.get 0
    any.convert_extern
    extern.convert_any
  )
)

;; CHECK: (module
;; NEXT: (type (;0;) (func (param i32) (result i31ref)))
;; NEXT: (type (;1;) (func (result i32)))
;; NEXT: (type (;2;) (func (param anyref) (result i32)))
;; NEXT: (type (;3;) (func (result i31ref)))
;; NEXT: (type (;4;) (func (param anyref) (result i31ref)))
;; NEXT: (type (;5;) (func (result anyref)))
;; NEXT: (type (;6;) (func (result eqref)))
;; NEXT: (type (;7;) (func (param i32 externref)))
;; NEXT: (type (;8;) (func (param i32) (result externref)))
;; NEXT: (type (;9;) (func (param externref) (result externref)))
;; NEXT: (func (;0;) (type 2) (param anyref) (result i32)
;; NEXT: block (result i31ref) ;; label = @1
;; NEXT: local.get 0
;; NEXT: br_on_cast 0 (;@1;) anyref i31ref
;; NEXT: drop
;; NEXT: i32.const -1
;; NEXT: return
;; NEXT: end
;; NEXT: i31.get_u
;; NEXT: )
;; NEXT: (func (;1;) (type 2) (param anyref) (result i32)
;; NEXT: block (result anyref) ;; label = @1
;; NEXT: local.get 0
;; NEXT: br_on_cast_fail 0 (;@1;) anyref i31ref
;; NEXT: i31.get_u
;; NEXT: return
;; NEXT: end
;; NEXT: drop
;; NEXT: i32.const -1
;; NEXT: )
;; NEXT: (func (;2;) (type 1) (result i32)
;; NEXT: i32.const 42
;; NEXT: ref.i31
;; NEXT: ref.cast (ref i31)
;; NEXT: i31.get_u
;; NEXT: )
;; NEXT: (func (;3;) (type 7) (param i32 externref)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: any.convert_extern
;; NEXT: table.set $t
;; NEXT: )
;; NEXT: (func (;4;) (type 1) (result i32)
;; NEXT: i32.const 42
;; NEXT: ref.i31
;; NEXT: i31.get_u
;; NEXT: )
;; NEXT: (func (;5;) (type 1) (result i32)
;; NEXT: i32.const -1
;; NEXT: ref.i31
;; NEXT: i31.get_s
;; NEXT: )
;; NEXT: (func (;6;) (type 1) (result i32)
;; NEXT: i32.const 100
;; NEXT: ref.i31
;; NEXT: i31.get_s
;; NEXT: )
;; NEXT: (func (;7;) (type 1) (result i32)
;; NEXT: i32.const 1073741823
;; NEXT: ref.i31
;; NEXT: i31.get_s
;; NEXT: )
;; NEXT: (func (;8;) (type 1) (result i32)
;; NEXT: i32.const 100
;; NEXT: ref.i31
;; NEXT: i31.get_u
;; NEXT: )
;; NEXT: (func (;9;) (type 1) (result i32)
;; NEXT: i32.const 2147483647
;; NEXT: ref.i31
;; NEXT: i31.get_u
;; NEXT: )
;; NEXT: (func (;10;) (type 1) (result i32)
;; NEXT: i32.const 42
;; NEXT: ref.i31
;; NEXT: ref.test i31ref
;; NEXT: )
;; NEXT: (func (;11;) (type 8) (param i32) (result externref)
;; NEXT: local.get 0
;; NEXT: table.get $t
;; NEXT: extern.convert_any
;; NEXT: )
;; NEXT: (func (;12;) (type 9) (param externref) (result externref)
;; NEXT: local.get 0
;; NEXT: any.convert_extern
;; NEXT: extern.convert_any
;; NEXT: )
;; NEXT: (func (;13;) (type 0) (param i32) (result i31ref)
;; NEXT: local.get 0
;; NEXT: ref.i31
;; NEXT: )
;; NEXT: (func (;14;) (type 2) (param anyref) (result i32)
;; NEXT: local.get 0
;; NEXT: ref.test i31ref
;; NEXT: )
;; NEXT: (func (;15;) (type 1) (result i32)
;; NEXT: ref.null any
;; NEXT: ref.test i31ref
;; NEXT: )
;; NEXT: (func (;16;) (type 1) (result i32)
;; NEXT: ref.null any
;; NEXT: ref.test (ref i31)
;; NEXT: )
;; NEXT: (func (;17;) (type 3) (result i31ref)
;; NEXT: ref.null any
;; NEXT: ref.cast i31ref
;; NEXT: )
;; NEXT: (func (;18;) (type 4) (param anyref) (result i31ref)
;; NEXT: local.get 0
;; NEXT: ref.cast i31ref
;; NEXT: )
;; NEXT: (func (;19;) (type 1) (result i32)
;; NEXT: ref.null any
;; NEXT: ref.is_null
;; NEXT: )
;; NEXT: (func (;20;) (type 5) (result anyref)
;; NEXT: ref.null any
;; NEXT: )
;; NEXT: (func (;21;) (type 3) (result i31ref)
;; NEXT: ref.null i31
;; NEXT: )
;; NEXT: (func (;22;) (type 6) (result eqref)
;; NEXT: ref.null eq
;; NEXT: )
;; NEXT: (table $t (;0;) 10 anyref)
;; NEXT: (export "make_i31" (func 13))
;; NEXT: (export "roundtrip_pos" (func 4))
;; NEXT: (export "roundtrip_neg" (func 5))
;; NEXT: (export "get_s_positive" (func 6))
;; NEXT: (export "get_s_max_positive" (func 7))
;; NEXT: (export "get_u_basic" (func 8))
;; NEXT: (export "get_u_max" (func 9))
;; NEXT: (export "test_is_i31" (func 14))
;; NEXT: (export "test_null_nullable" (func 15))
;; NEXT: (export "test_null_nonnullable" (func 16))
;; NEXT: (export "test_i31_is_i31" (func 10))
;; NEXT: (export "cast_i31_to_i31" (func 2))
;; NEXT: (export "cast_null_nullable" (func 17))
;; NEXT: (export "cast_any_to_i31" (func 18))
;; NEXT: (export "br_on_cast_i31" (func 0))
;; NEXT: (export "br_on_cast_fail_i31" (func 1))
;; NEXT: (export "null_any" (func 20))
;; NEXT: (export "null_i31" (func 21))
;; NEXT: (export "null_eq" (func 22))
;; NEXT: (export "null_any_is_null" (func 19))
;; NEXT: (export "store_extern_as_any" (func 3))
;; NEXT: (export "load_any_as_extern" (func 11))
;; NEXT: (export "roundtrip_extern" (func 12))
