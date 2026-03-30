;; Regression test for https://github.com/wasm-bindgen/wasm-bindgen/issues/5065
;;
;; The Rust compiler can emit `__wbindgen_malloc` (type: (i32,i32)->i32) and
;; `__wbindgen_realloc` (type: (i32,i32,i32,i32)->i32) in either order in the
;; wasm type section depending on the compiler version.  After walrus
;; round-trips the binary the type indices must be stable regardless of which
;; order appeared in the input.
;;
;; This variant has malloc's type FIRST (as emitted by one compiler version).
;; See type-order-stability-swapped.wat for the other ordering.
;; Both must produce the same CHECK output.

(module
  (type (func (param i32 i32) (result i32)))
  (type (func (param i32 i32 i32 i32) (result i32)))

  (func $malloc  (type 0) (param i32 i32) (result i32) i32.const 0)
  (func $realloc (type 1) (param i32 i32 i32 i32) (result i32) i32.const 0)

  (export "__wbindgen_malloc"  (func $malloc))
  (export "__wbindgen_realloc" (func $realloc))
)

;; CHECK: (type (;0;) (func (param i32 i32) (result i32)))
;; CHECK: (type (;1;) (func (param i32 i32 i32 i32) (result i32)))
;; CHECK: (export "__wbindgen_malloc" (func $malloc))
;; CHECK: (export "__wbindgen_realloc" (func $realloc))
