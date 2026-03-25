(module
  ;; Self-referential struct: a linked-list node.
  ;; This is an implicit singleton rec group that references its own type index.
  (type $node (struct (field $val i32) (field $next (mut (ref null $node)))))

  ;; Create a single-element list
  (func (export "singleton") (param i32) (result (ref $node))
    local.get 0
    ref.null $node
    struct.new $node
  )

  ;; Get the value from a node
  (func (export "get_val") (param (ref null $node)) (result i32)
    local.get 0
    struct.get $node $val
  )

  ;; Get the next pointer from a node
  (func (export "get_next") (param (ref null $node)) (result (ref null $node))
    local.get 0
    struct.get $node $next
  )

  ;; Set the next pointer on a node
  (func (export "set_next") (param (ref null $node) (ref null $node))
    local.get 0
    local.get 1
    struct.set $node $next
  )

  ;; Function local with self-referential type
  (func (export "make_null") (result (ref null $node))
    (local $n (ref null $node))
    local.get $n
  )
)

;; CHECK: (module
;; NEXT: (type $node (;0;) (struct (field i32) (field (mut (ref null 0)))))
;; NEXT: (type (;1;) (func (param i32) (result (ref 0))))
;; NEXT: (type (;2;) (func (param (ref null 0)) (result i32)))
;; NEXT: (type (;3;) (func (param (ref null 0)) (result (ref null 0))))
;; NEXT: (type (;4;) (func (param (ref null 0) (ref null 0))))
;; NEXT: (type (;5;) (func (result (ref null 0))))
;; NEXT: (func (;0;) (type 1) (param i32) (result (ref 0))
;; NEXT: local.get 0
;; NEXT: ref.null 0
;; NEXT: struct.new $node
;; NEXT: )
;; NEXT: (func (;1;) (type 4) (param (ref null 0) (ref null 0))
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: struct.set $node 1
;; NEXT: )
;; NEXT: (func (;2;) (type 2) (param (ref null 0)) (result i32)
;; NEXT: local.get 0
;; NEXT: struct.get $node 0
;; NEXT: )
;; NEXT: (func (;3;) (type 3) (param (ref null 0)) (result (ref null 0))
;; NEXT: local.get 0
;; NEXT: struct.get $node 1
;; NEXT: )
;; NEXT: (func (;4;) (type 5) (result (ref null 0))
;; NEXT: (local (ref null 0))
;; NEXT: local.get 0
;; NEXT: )
;; NEXT: (export "singleton" (func 0))
;; NEXT: (export "get_val" (func 2))
;; NEXT: (export "get_next" (func 3))
;; NEXT: (export "set_next" (func 1))
;; NEXT: (export "make_null" (func 4))
