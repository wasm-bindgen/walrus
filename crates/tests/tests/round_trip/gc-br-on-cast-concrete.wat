;; Verify that concrete heap types in br_on_cast / br_on_cast_fail survive the
;; GC pass and round-trip correctly.

(module
  (type $base  (sub (struct (field i32))))
  (type $child (sub final $base (struct (field i32) (field f64))))

  ;; br_on_cast: branch if $base value is actually a $child
  (func (export "is_child") (param (ref null $base)) (result i32)
    (block $yes (result (ref null $child))
      local.get 0
      br_on_cast $yes (ref null $base) (ref null $child)
      ;; cast failed — not a child
      i32.const 0
      return
    )
    ;; cast succeeded — pop the $child ref and return 1
    drop
    i32.const 1
  )

  ;; br_on_cast_fail: branch if $base value is NOT a $child
  (func (export "is_not_child") (param (ref null $base)) (result i32)
    (block $no (result (ref null $base))
      local.get 0
      br_on_cast_fail $no (ref null $base) (ref null $child)
      ;; cast succeeded — it IS a child
      drop
      i32.const 1
      return
    )
    ;; cast failed — not a child
    drop
    i32.const 0
  )
)

;; CHECK: (type $base (;0;) (sub (struct (field i32))))
;; CHECK: (type $child (;1;) (sub final $base (struct (field i32) (field f64))))
;; CHECK: br_on_cast_fail
;; CHECK: br_on_cast 0
;; CHECK: (export "is_child"
;; CHECK: (export "is_not_child"
