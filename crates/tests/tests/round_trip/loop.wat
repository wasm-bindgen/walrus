(module
  (type (;0;) (func))
  (func $f (type 0)
    loop
    end)
  (export "inf_loop" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (func $f (;0;) (type 0)
      loop ;; label = @1
      end
    )
    (export "inf_loop" (func $f))
;)
