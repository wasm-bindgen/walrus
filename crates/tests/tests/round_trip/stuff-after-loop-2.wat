(module
  (type (;0;) (func (result i32)))
  (func $f (type 0)
    loop
      br 0
    end
    i32.const 1)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      loop ;; label = @1
        br 0 (;@1;)
      end
      i32.const 1
    )
    (export "f" (func $f))
;)
