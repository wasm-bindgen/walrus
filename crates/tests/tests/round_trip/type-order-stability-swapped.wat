;; Regression test for https://github.com/wasm-bindgen/wasm-bindgen/issues/5065
;;
;; This variant has realloc's type FIRST (as emitted by the other compiler
;; version).  After walrus round-trips the binary the type indices must be
;; identical to type-order-stability.wat — the smaller type (malloc, 2 params)
;; must always end up at index 0.

(module
  (type (func (param i32 i32 i32 i32) (result i32)))
  (type (func (param i32 i32) (result i32)))

  (func $malloc  (type 1) (param i32 i32) (result i32) i32.const 0)
  (func $realloc (type 0) (param i32 i32 i32 i32) (result i32) i32.const 0)

  (export "__wbindgen_malloc"  (func $malloc))
  (export "__wbindgen_realloc" (func $realloc))
)

;; CHECK: (type (;0;) (func (param i32 i32) (result i32)))
;; CHECK: (type (;1;) (func (param i32 i32 i32 i32) (result i32)))
;; CHECK: (export "__wbindgen_malloc" (func $malloc))
;; CHECK: (export "__wbindgen_realloc" (func $realloc))
