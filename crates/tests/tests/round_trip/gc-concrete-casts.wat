;; Tests ref.test, ref.cast, br_on_cast, br_on_cast_fail with CONCRETE heap types.
;; All existing tests only use abstract types (i31, any). This exercises the
;; HeapType::Concrete(TypeId) code path.

(module
  ;; Base struct (non-final)
  (type $base (sub (struct (field i32))))

  ;; Derived struct (final, extends $base)
  (type $derived (sub final $base (struct (field i32) (field f64))))

  ;; ref.test with non-nullable concrete type
  (func (export "test_nonnull") (param (ref null $base)) (result i32)
    local.get 0
    ref.test (ref $derived)
  )

  ;; ref.test with nullable concrete type
  (func (export "test_nullable") (param (ref null $base)) (result i32)
    local.get 0
    ref.test (ref null $derived)
  )

  ;; ref.cast with non-nullable concrete type
  (func (export "cast_nonnull") (param (ref null $base)) (result (ref $derived))
    local.get 0
    ref.cast (ref $derived)
  )

  ;; ref.cast with nullable concrete type
  (func (export "cast_nullable") (param (ref null $base)) (result (ref null $derived))
    local.get 0
    ref.cast (ref null $derived)
  )

  ;; br_on_cast from (ref null $base) to (ref null $derived)
  (func (export "br_cast_derived") (param (ref null $base)) (result i32)
    (block $is_derived (result (ref null $derived))
      local.get 0
      br_on_cast $is_derived (ref null $base) (ref null $derived)
      drop
      i32.const -1
      return
    )
    struct.get $derived 0
  )

  ;; br_on_cast_fail from (ref null $base) to (ref null $derived)
  (func (export "br_cast_fail_derived") (param (ref null $base)) (result i32)
    (block $not_derived (result (ref null $base))
      local.get 0
      br_on_cast_fail $not_derived (ref null $base) (ref null $derived)
      struct.get $derived 0
      return
    )
    struct.get $base 0
  )
)

(; CHECK-ALL:
  (module
    (type $base (;0;) (sub (struct (field i32))))
    (type $derived (;1;) (sub final $base (struct (field i32) (field f64))))
    (type (;2;) (func (param (ref null 0)) (result i32)))
    (type (;3;) (func (param (ref null 0)) (result (ref 1))))
    (type (;4;) (func (param (ref null 0)) (result (ref null 1))))
    (func (;0;) (type 2) (param (ref null 0)) (result i32)
      block (result (ref null 1)) ;; label = @1
        local.get 0
        br_on_cast 0 (;@1;) (ref null 0) (ref null 1)
        drop
        i32.const -1
        return
      end
      struct.get $derived 0
    )
    (func (;1;) (type 2) (param (ref null 0)) (result i32)
      block (result (ref null 0)) ;; label = @1
        local.get 0
        br_on_cast_fail 0 (;@1;) (ref null 0) (ref null 1)
        struct.get $derived 0
        return
      end
      struct.get $base 0
    )
    (func (;2;) (type 2) (param (ref null 0)) (result i32)
      local.get 0
      ref.test (ref 1)
    )
    (func (;3;) (type 2) (param (ref null 0)) (result i32)
      local.get 0
      ref.test (ref null 1)
    )
    (func (;4;) (type 3) (param (ref null 0)) (result (ref 1))
      local.get 0
      ref.cast (ref 1)
    )
    (func (;5;) (type 4) (param (ref null 0)) (result (ref null 1))
      local.get 0
      ref.cast (ref null 1)
    )
    (export "test_nonnull" (func 2))
    (export "test_nullable" (func 3))
    (export "cast_nonnull" (func 4))
    (export "cast_nullable" (func 5))
    (export "br_cast_derived" (func 0))
    (export "br_cast_fail_derived" (func 1))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
