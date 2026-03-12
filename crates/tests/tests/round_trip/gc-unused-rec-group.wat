;; Verify the GC pass eliminates an entirely unused rec group.

(module
  ;; Used singleton struct
  (type $used (struct (field i32)))

  ;; Unused rec group — both types should be eliminated
  (rec
    (type $unused_a (struct (field (ref null $unused_b))))
    (type $unused_b (struct (field (ref null $unused_a))))
  )

  ;; Function that uses $used
  (func (export "make") (param i32) (result (ref $used))
    local.get 0
    struct.new $used
  )
)

(; CHECK-ALL:
  (module
    (type $used (;0;) (struct (field i32)))
    (type (;1;) (func (param i32) (result (ref 0))))
    (func (;0;) (type 1) (param i32) (result (ref 0))
      local.get 0
      struct.new $used
    )
    (export "make" (func 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
