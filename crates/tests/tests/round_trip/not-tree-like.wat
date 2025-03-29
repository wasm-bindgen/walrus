;; Non-tree-like, stack-y expressions.

(module
  (import "env" "blackbox" (func $blackbox (param i32) (result i32)))
  (func (export "$f") (result i32)
    (call $blackbox (i32.const 1))
    (drop (call $blackbox (i32.const 2)))
    (call $blackbox (i32.const 3))
    i32.add
  ))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (type (;1;) (func (param i32) (result i32)))
    (import "env" "blackbox" (func $blackbox (;0;) (type 1)))
    (func (;1;) (type 0) (result i32)
      i32.const 1
      call $blackbox
      i32.const 2
      call $blackbox
      drop
      i32.const 3
      call $blackbox
      i32.add
    )
    (export "$f" (func 1))
;)
