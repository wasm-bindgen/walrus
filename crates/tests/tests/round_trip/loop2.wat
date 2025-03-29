(module
  (func $dummy)
  (func (export "as-loop-last") (param i32)
    (loop
      (call $dummy)
      (br_if 1 (local.get 0)))
  ))

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (type (;1;) (func (param i32)))
    (func (;0;) (type 1) (param i32)
      loop ;; label = @1
        call $dummy
        local.get 0
        br_if 1 (;@0;)
      end
    )
    (func $dummy (;1;) (type 0))
    (export "as-loop-last" (func 0))
;)
