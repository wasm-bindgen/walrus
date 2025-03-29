(module
  (type (;0;) (func (param i32) (result i32)))
  (func $f (type 0)
    block
      block
        block
          local.get 0
          br_table 0 1 2
        end
        i32.const 300
        return
      end
      i32.const 200
      return
    end
    i32.const 100)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32) (result i32)))
    (func $f (;0;) (type 0) (param i32) (result i32)
      block ;; label = @1
        block ;; label = @2
          block ;; label = @3
            local.get 0
            br_table 0 (;@3;) 1 (;@2;) 2 (;@1;)
          end
          i32.const 300
          return
        end
        i32.const 200
        return
      end
      i32.const 100
    )
    (export "f" (func $f))
;)