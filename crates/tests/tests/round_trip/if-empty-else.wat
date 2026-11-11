;; An empty `else` arm is not emitted; a non-empty one is kept.
(module
  (func $no_else (export "no_else") (param i32)
    local.get 0
    if
      unreachable
    end)
  (func $empty_else (export "empty_else") (param i32)
    local.get 0
    if
      unreachable
    else
    end)
  (func $params_eq_results (export "params_eq_results") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    if (param i32) (result i32)
      i32.const 1
      i32.add
    else
    end)
  (func $nested (export "nested") (param i32)
    local.get 0
    if
      local.get 0
      if
        unreachable
      else
      end
    else
      local.get 0
      if
        unreachable
      end
    end))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32)))
    (type (;1;) (func (param i32) (result i32)))
    (type (;2;) (func (param i32 i32) (result i32)))
    (export "no_else" (func $no_else))
    (export "empty_else" (func $empty_else))
    (export "params_eq_results" (func $params_eq_results))
    (export "nested" (func $nested))
    (func $nested (;0;) (type 0) (param i32)
      local.get 0
      if ;; label = @1
        local.get 0
        if ;; label = @2
          unreachable
        end
      else
        local.get 0
        if ;; label = @2
          unreachable
        end
      end
    )
    (func $params_eq_results (;1;) (type 2) (param i32 i32) (result i32)
      local.get 0
      local.get 1
      if (type 1) (param i32) (result i32) ;; label = @1
        i32.const 1
        i32.add
      end
    )
    (func $no_else (;2;) (type 0) (param i32)
      local.get 0
      if ;; label = @1
        unreachable
      end
    )
    (func $empty_else (;3;) (type 0) (param i32)
      local.get 0
      if ;; label = @1
        unreachable
      end
    )
  )
;)
