;; Verify the GC pass eliminates unused struct/array types but keeps used ones.

(module
  ;; Used struct type (referenced by exported function)
  (type $used_struct (struct (field (mut i32))))

  ;; Unused struct type — should be eliminated
  (type $unused_struct (struct (field f64) (field f64)))

  ;; Unused array type — should be eliminated
  (type $unused_array (array (mut i32)))

  ;; Function that uses $used_struct
  (func (export "make") (param i32) (result (ref $used_struct))
    local.get 0
    struct.new $used_struct
  )
)

(; CHECK-ALL:
  (module
    (type $used_struct (;0;) (struct (field (mut i32))))
    (type (;1;) (func (param i32) (result (ref 0))))
    (func (;0;) (type 1) (param i32) (result (ref 0))
      local.get 0
      struct.new $used_struct
    )
    (export "make" (func 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
