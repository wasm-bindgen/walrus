;; Test multiple expressions in if/else arms.

(module
  (type (;0;) (func (param i32) (result i32)))
  (func $if_else (type 0)
    local.get 0
    if (result i32)
      (local.set 0 (i32.const 2))
      i32.const 1
    else
      (local.set 0 (i32.const 1))
      i32.const 2
    end)
  (export "if_else" (func $if_else)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32) (result i32)))
    (func $if_else (;0;) (type 0) (param i32) (result i32)
      local.get 0
      if (result i32) ;; label = @1
        i32.const 2
        local.set 0
        i32.const 1
      else
        i32.const 1
        local.set 0
        i32.const 2
      end
    )
    (export "if_else" (func $if_else))
;)
