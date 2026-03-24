;; Edge cases for GC types: empty struct, deep subtype chain, large rec group,
;; struct with every storage type, array of concrete refs, mixed module.

(module
  ;; Empty struct
  (type $empty (struct))

  ;; Deep subtype chain (4 levels)
  (type $a (sub (struct (field i32))))
  (type $b (sub $a (struct (field i32) (field i64))))
  (type $c (sub $b (struct (field i32) (field i64) (field f32))))
  (type $d (sub final $c (struct (field i32) (field i64) (field f32) (field f64))))

  ;; Large rec group (4 mutually-referencing types)
  (rec
    (type $node1 (struct (field (ref null $node2)) (field (ref null $node4))))
    (type $node2 (struct (field (ref null $node3)) (field (ref null $node1))))
    (type $node3 (struct (field (ref null $node4)) (field (ref null $node2))))
    (type $node4 (struct (field (ref null $node1)) (field (ref null $node3))))
  )

  ;; Struct with every storage type
  (type $kitchen_sink (struct
    (field (mut i8))       ;; packed i8
    (field (mut i16))      ;; packed i16
    (field (mut i32))
    (field (mut i64))
    (field (mut f32))
    (field (mut f64))
    (field (mut funcref))
    (field (mut externref))
    (field (mut anyref))
  ))

  ;; Array of nullable concrete refs
  (type $node_array (array (mut (ref null $node1))))

  ;; Keep empty struct alive
  (func (export "make_empty") (result (ref $empty))
    struct.new $empty
  )

  ;; Exercise deepest subtype
  (func (export "make_d") (param i32 i64 f32 f64) (result (ref $d))
    local.get 0
    local.get 1
    local.get 2
    local.get 3
    struct.new $d
  )

  ;; Read field from base type
  (func (export "get_a_field") (param (ref null $a)) (result i32)
    local.get 0
    struct.get $a 0
  )

  ;; Create a node1 (part of large rec group)
  (func (export "make_node1") (param (ref null $node2) (ref null $node4)) (result (ref $node1))
    local.get 0
    local.get 1
    struct.new $node1
  )

  ;; Create array of nodes
  (func (export "make_node_array") (param i32) (result (ref $node_array))
    ref.null $node1
    local.get 0
    array.new $node_array
  )

  ;; Kitchen sink: create and read packed fields
  (func (export "kitchen_sink_packed") (result i32)
    i32.const 42        ;; i8
    i32.const 1000      ;; i16
    i32.const 0         ;; i32
    i64.const 0         ;; i64
    f32.const 0         ;; f32
    f64.const 0         ;; f64
    ref.null func       ;; funcref
    ref.null extern     ;; externref
    ref.null any        ;; anyref
    struct.new $kitchen_sink
    struct.get_u $kitchen_sink 0
  )

  ;; Regular non-GC function to test mixed modules
  (memory 1)
  (func (export "regular_func") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
)

;; CHECK: (module
;; NEXT: (type $empty (;0;) (struct))
;; NEXT: (type $a (;1;) (sub (struct (field i32))))
;; NEXT: (type $b (;2;) (sub $a (struct (field i32) (field i64))))
;; NEXT: (type $c (;3;) (sub $b (struct (field i32) (field i64) (field f32))))
;; NEXT: (type $d (;4;) (sub final $c (struct (field i32) (field i64) (field f32) (field f64))))
;; NEXT: (rec
;; NEXT: (type $node1 (;5;) (struct (field (ref null 6)) (field (ref null 8))))
;; NEXT: (type $node2 (;6;) (struct (field (ref null 7)) (field (ref null 5))))
;; NEXT: (type $node3 (;7;) (struct (field (ref null 8)) (field (ref null 6))))
;; NEXT: (type $node4 (;8;) (struct (field (ref null 5)) (field (ref null 7))))
;; NEXT: )
;; NEXT: (type $kitchen_sink (;9;) (struct (field (mut i8)) (field (mut i16)) (field (mut i32)) (field (mut i64)) (field (mut f32)) (field (mut f64)) (field (mut funcref)) (field (mut externref)) (field (mut anyref))))
;; NEXT: (type $node_array (;10;) (array (mut (ref null 5))))
;; NEXT: (type (;11;) (func (result (ref 0))))
;; NEXT: (type (;12;) (func (param i32 i64 f32 f64) (result (ref 4))))
;; NEXT: (type (;13;) (func (param (ref null 1)) (result i32)))
;; NEXT: (type (;14;) (func (param (ref null 6) (ref null 8)) (result (ref 5))))
;; NEXT: (type (;15;) (func (param i32) (result (ref 10))))
;; NEXT: (type (;16;) (func (result i32)))
;; NEXT: (type (;17;) (func (param i32 i32) (result i32)))
;; NEXT: (func (;0;) (type 16) (result i32)
;; NEXT: i32.const 42
;; NEXT: i32.const 1000
;; NEXT: i32.const 0
;; NEXT: i64.const 0
;; NEXT: f32.const 0x0p+0 (;=0;)
;; NEXT: f64.const 0x0p+0 (;=0;)
;; NEXT: ref.null func
;; NEXT: ref.null extern
;; NEXT: ref.null any
;; NEXT: struct.new $kitchen_sink
;; NEXT: struct.get_u $kitchen_sink 0
;; NEXT: )
;; NEXT: (func (;1;) (type 12) (param i32 i64 f32 f64) (result (ref 4))
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: local.get 2
;; NEXT: local.get 3
;; NEXT: struct.new $d
;; NEXT: )
;; NEXT: (func (;2;) (type 14) (param (ref null 6) (ref null 8)) (result (ref 5))
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: struct.new $node1
;; NEXT: )
;; NEXT: (func (;3;) (type 15) (param i32) (result (ref 10))
;; NEXT: ref.null 5
;; NEXT: local.get 0
;; NEXT: array.new $node_array
;; NEXT: )
;; NEXT: (func (;4;) (type 17) (param i32 i32) (result i32)
;; NEXT: local.get 0
;; NEXT: local.get 1
;; NEXT: i32.add
;; NEXT: )
;; NEXT: (func (;5;) (type 13) (param (ref null 1)) (result i32)
;; NEXT: local.get 0
;; NEXT: struct.get $a 0
;; NEXT: )
;; NEXT: (func (;6;) (type 11) (result (ref 0))
;; NEXT: struct.new $empty
;; NEXT: )
;; NEXT: (export "make_empty" (func 6))
;; NEXT: (export "make_d" (func 1))
;; NEXT: (export "get_a_field" (func 5))
;; NEXT: (export "make_node1" (func 2))
;; NEXT: (export "make_node_array" (func 3))
;; NEXT: (export "kitchen_sink_packed" (func 0))
;; NEXT: (export "regular_func" (func 4))
