(module
  (type (;0;) (func (result i32)))
  (func $f (type 0) (local i32)
    (local.set 0 (i32.const 9))
    loop
      (br_if 0 (i32.eqz (local.get 0)))
      (local.set 0 (i32.add (local.get 0) (i32.const 1)))
    end
    i32.const 10)
  (export "count_to_ten" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      (local i32)
      i32.const 9
      local.set 0
      loop ;; label = @1
        local.get 0
        i32.eqz
        br_if 0 (;@1;)
        local.get 0
        i32.const 1
        i32.add
        local.set 0
      end
      i32.const 10
    )
    (export "count_to_ten" (func $f))
;)