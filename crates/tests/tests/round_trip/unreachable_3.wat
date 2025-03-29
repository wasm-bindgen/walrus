;; Instructions after an unreachable should not be emitted, but unreachable
;; instructions outside the block should be.

(module
  (type (;0;) (func (result i32)))
  (func $f (type 0) (result i32)
    block
      unreachable
    end
    i32.const 42)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      block ;; label = @1
        unreachable
      end
      i32.const 42
    )
    (export "f" (func $f))
;)
