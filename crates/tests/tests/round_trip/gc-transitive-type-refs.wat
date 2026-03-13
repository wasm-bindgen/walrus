;; Verify the GC pass keeps transitively-referenced GC types.
;; Struct $outer references $inner in a field. Only $outer is directly used.
;; Both must survive.

(module
  ;; Inner type — only referenced transitively via $outer's field
  (type $inner (struct (field i32) (field i64)))

  ;; Outer type — directly used, has a field referencing $inner
  (type $outer (struct (field (ref null $inner)) (field i32)))

  ;; Unrelated unused type — should be eliminated
  (type $unrelated (struct (field f32)))

  ;; Function uses $outer directly; $inner is only reachable through the field type
  (func (export "make_outer") (param (ref null $inner) i32) (result (ref $outer))
    local.get 0
    local.get 1
    struct.new $outer
  )

  (func (export "get_inner") (param (ref null $outer)) (result (ref null $inner))
    local.get 0
    struct.get $outer 0
  )
)

(; CHECK-ALL:
  (module
    (type $inner (;0;) (struct (field i32) (field i64)))
    (type $outer (;1;) (struct (field (ref null 0)) (field i32)))
    (type (;2;) (func (param (ref null 0) i32) (result (ref 1))))
    (type (;3;) (func (param (ref null 1)) (result (ref null 0))))
    (func (;0;) (type 2) (param (ref null 0) i32) (result (ref 1))
      local.get 0
      local.get 1
      struct.new $outer
    )
    (func (;1;) (type 3) (param (ref null 1)) (result (ref null 0))
      local.get 0
      struct.get $outer 0
    )
    (export "make_outer" (func 0))
    (export "get_inner" (func 1))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
