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

;; CHECK: (module
;; NEXT: (rec
;; NEXT: (type $tree (;0;) (struct (field i32) (field (ref null 1))))
;; NEXT: (type $list (;1;) (struct (field (ref null 0)) (field (ref null 1))))
;; NEXT: )
;; NEXT: (rec
;; NEXT: (type $self_ref (;2;) (struct (field (ref null 2))))
;; NEXT: )
;; NEXT: (type $simple (;3;) (struct (field i32)))
;; NEXT: (type (;4;) (func (param (ref null 0) (ref null 1) (ref null 2) (ref null 3)) (result i32)))
;; NEXT: (func (;0;) (type 4) (param (ref null 0) (ref null 1) (ref null 2) (ref null 3)) (result i32)
;; NEXT: i32.const 0
;; NEXT: )
;; NEXT: (export "use_types" (func 0))
