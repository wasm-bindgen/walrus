;; Verify that when one member of a rec group is used, all members survive.
;; Rec groups are atomic — you can't GC half a rec group.

(module
  ;; Used rec group — only $tree is directly used, but $leaf and $branch
  ;; must also survive since they're in the same rec group.
  (rec
    (type $tree (sub (struct (field i32))))
    (type $leaf (sub final $tree (struct (field i32) (field f64))))
    (type $branch (sub final $tree (struct (field i32) (field (ref null $tree)) (field (ref null $tree)))))
  )

  ;; Unrelated unused struct
  (type $unused (struct (field i64)))

  ;; Only uses $tree, but $leaf and $branch must survive
  (func (export "get_tag") (param (ref null $tree)) (result i32)
    local.get 0
    struct.get $tree 0
  )
)

(; CHECK-ALL:
  (module
    (rec
      (type $tree (;0;) (sub (struct (field i32))))
      (type $leaf (;1;) (sub final $tree (struct (field i32) (field f64))))
      (type $branch (;2;) (sub final $tree (struct (field i32) (field (ref null 0)) (field (ref null 0)))))
    )
    (type (;3;) (func (param (ref null 0)) (result i32)))
    (func (;0;) (type 3) (param (ref null 0)) (result i32)
      local.get 0
      struct.get $tree 0
    )
    (export "get_tag" (func 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
