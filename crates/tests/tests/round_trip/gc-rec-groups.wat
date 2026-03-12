(module
  ;; Explicit rec group with mutually recursive types
  (rec
    (type $tree (struct (field i32) (field (ref null $list))))
    (type $list (struct (field (ref null $tree)) (field (ref null $list))))
  )

  ;; Another explicit rec group (singleton)
  (rec
    (type $self_ref (struct (field (ref null $self_ref))))
  )

  ;; Implicit singleton (no rec wrapper)
  (type $simple (struct (field i32)))

  ;; Function that uses these types to keep them alive
  (func (export "use_types") (param (ref null $tree)) (param (ref null $list)) (param (ref null $self_ref)) (param (ref null $simple)) (result i32)
    i32.const 0
  )
)

(; CHECK-ALL:
  (module
    (rec
      (type $tree (;0;) (struct (field i32) (field (ref null 1))))
      (type $list (;1;) (struct (field (ref null 0)) (field (ref null 1))))
    )
    (rec
      (type $self_ref (;2;) (struct (field (ref null 2))))
    )
    (type $simple (;3;) (struct (field i32)))
    (type (;4;) (func (param (ref null 0) (ref null 1) (ref null 2) (ref null 3)) (result i32)))
    (func (;0;) (type 4) (param (ref null 0) (ref null 1) (ref null 2) (ref null 3)) (result i32)
      i32.const 0
    )
    (export "use_types" (func 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
