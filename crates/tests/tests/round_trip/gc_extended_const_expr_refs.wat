;; Regression test: Extended const expressions in element segment items
;; must have their referenced objects (globals, types) survive GC.
;;
;; Previously, ConstExpr::Extended was caught by a wildcard in the element
;; segment processing, causing referenced globals and types to be
;; incorrectly garbage collected.

(module
  ;; A struct type referenced ONLY through an extended element expression.
  ;; Must survive GC.
  (type $pair (struct (field i32) (field i32)))

  ;; An unused type that should be removed by GC.
  (type $unused_type (struct (field i64)))

  ;; A global referenced ONLY through an extended element expression.
  ;; Must survive GC.
  (global $elem_base i32 (i32.const 100))

  ;; An unused global that should be removed by GC.
  (global $unused_global i32 (i32.const 999))

  (table $t 10 anyref)

  ;; Active element segment with Extended const expression items:
  ;;   (ref.i31 (global.get $elem_base))       -> Extended [GlobalGet, RefI31]
  ;;   (struct.new $pair (i32.const 1) (...))   -> Extended [I32Const, I32Const, StructNew]
  ;; Both reference objects that must survive GC.
  (elem (table $t) (offset (i32.const 0)) anyref
    (item (ref.i31 (global.get $elem_base)))
    (item (struct.new $pair (i32.const 1) (i32.const 2)))
  )

  ;; Export a function to root the table (and transitively the element segment).
  (func (export "get_elem") (result anyref)
    i32.const 0
    table.get $t
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func (result anyref)))
    (type $pair (;1;) (struct (field i32) (field i32)))
    (table $t (;0;) 10 anyref)
    (global $elem_base (;0;) i32 i32.const 100)
    (export "get_elem" (func 0))
    (elem (;0;) (table $t) (i32.const 0) anyref (item global.get $elem_base ref.i31) (item i32.const 1 i32.const 2 struct.new $pair))
    (func (;0;) (type 0) (result anyref)
      i32.const 0
      table.get $t
    )
  )
;)
