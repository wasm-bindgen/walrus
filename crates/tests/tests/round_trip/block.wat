(module
  (type (;0;) (func (result i32)))
  (func $f (type 0) (local i32)
    i32.const 0
    drop
    block
      i32.const 1
      drop
    end
    i32.const 2)

  (func (export "foo") (result i32)
    block (result i32)
     i32.const 0
    end)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      i32.const 0
      drop
      block ;; label = @1
        i32.const 1
        drop
      end
      i32.const 2
    )
    (func (;1;) (type 0) (result i32)
      block (result i32) ;; label = @1
        i32.const 0
      end
    )
    (export "foo" (func 1))
    (export "f" (func $f))
;)