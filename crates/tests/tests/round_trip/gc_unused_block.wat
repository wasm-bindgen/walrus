;; Can remove unused blocks.

(module
  (type (;0;) (func (result i32)))
  (func $f (type 0) (result i32)
    (block
      br 0
      (loop
        ;; This loop block is unreachable, and should get GC'd.
        nop))
    i32.const 42)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      block ;; label = @1
        br 0 (;@1;)
      end
      i32.const 42
    )
    (export "f" (func $f))
;)