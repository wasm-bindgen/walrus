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

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (type (;1;) (func (result anyref)))
    (type (;2;) (func (result eqref)))
    (type (;3;) (func (result i31ref)))
    (type (;4;) (func (param i32) (result externref)))
    (type (;5;) (func (param i32) (result i31ref)))
    (type (;6;) (func (param i32 externref)))
    (type (;7;) (func (param externref) (result externref)))
    (type (;8;) (func (param anyref) (result i32)))
    (type (;9;) (func (param anyref) (result i31ref)))
    (func (;0;) (type 8) (param anyref) (result i32)
      block (result i31ref) ;; label = @1
        local.get 0
        br_on_cast 0 (;@1;) anyref i31ref
        drop
        i32.const -1
        return
      end
      i31.get_u
    )
    (func (;1;) (type 8) (param anyref) (result i32)
      block (result anyref) ;; label = @1
        local.get 0
        br_on_cast_fail 0 (;@1;) anyref i31ref
        i31.get_u
        return
      end
      drop
      i32.const -1
    )
    (func (;2;) (type 0) (result i32)
      i32.const 42
      ref.i31
      ref.cast (ref i31)
      i31.get_u
    )
    (func (;3;) (type 6) (param i32 externref)
      local.get 0
      local.get 1
      any.convert_extern
      table.set $t
    )
    (func (;4;) (type 0) (result i32)
      i32.const 42
      ref.i31
      i31.get_u
    )
    (func (;5;) (type 0) (result i32)
      i32.const -1
      ref.i31
      i31.get_s
    )
    (func (;6;) (type 0) (result i32)
      i32.const 100
      ref.i31
      i31.get_s
    )
    (func (;7;) (type 0) (result i32)
      i32.const 1073741823
      ref.i31
      i31.get_s
    )
    (func (;8;) (type 0) (result i32)
      i32.const 100
      ref.i31
      i31.get_u
    )
    (func (;9;) (type 0) (result i32)
      i32.const 2147483647
      ref.i31
      i31.get_u
    )
    (func (;10;) (type 0) (result i32)
      i32.const 42
      ref.i31
      ref.test i31ref
    )
    (func (;11;) (type 4) (param i32) (result externref)
      local.get 0
      table.get $t
      extern.convert_any
    )
    (func (;12;) (type 7) (param externref) (result externref)
      local.get 0
      any.convert_extern
      extern.convert_any
    )
    (func (;13;) (type 5) (param i32) (result i31ref)
      local.get 0
      ref.i31
    )
    (func (;14;) (type 8) (param anyref) (result i32)
      local.get 0
      ref.test i31ref
    )
    (func (;15;) (type 0) (result i32)
      ref.null any
      ref.test i31ref
    )
    (func (;16;) (type 0) (result i32)
      ref.null any
      ref.test (ref i31)
    )
    (func (;17;) (type 3) (result i31ref)
      ref.null any
      ref.cast i31ref
    )
    (func (;18;) (type 9) (param anyref) (result i31ref)
      local.get 0
      ref.cast i31ref
    )
    (func (;19;) (type 0) (result i32)
      ref.null any
      ref.is_null
    )
    (func (;20;) (type 1) (result anyref)
      ref.null any
    )
    (func (;21;) (type 3) (result i31ref)
      ref.null i31
    )
    (func (;22;) (type 2) (result eqref)
      ref.null eq
    )
    (table $t (;0;) 10 anyref)
    (export "make_i31" (func 13))
    (export "roundtrip_pos" (func 4))
    (export "roundtrip_neg" (func 5))
    (export "get_s_positive" (func 6))
    (export "get_s_max_positive" (func 7))
    (export "get_u_basic" (func 8))
    (export "get_u_max" (func 9))
    (export "test_is_i31" (func 14))
    (export "test_null_nullable" (func 15))
    (export "test_null_nonnullable" (func 16))
    (export "test_i31_is_i31" (func 10))
    (export "cast_i31_to_i31" (func 2))
    (export "cast_null_nullable" (func 17))
    (export "cast_any_to_i31" (func 18))
    (export "br_on_cast_i31" (func 0))
    (export "br_on_cast_fail_i31" (func 1))
    (export "null_any" (func 20))
    (export "null_i31" (func 21))
    (export "null_eq" (func 22))
    (export "null_any_is_null" (func 19))
    (export "store_extern_as_any" (func 3))
    (export "load_any_as_extern" (func 11))
    (export "roundtrip_extern" (func 12))
    (@producers
      (processed-by "walrus" "0.25.0")
    )
  )
;)
