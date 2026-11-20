;; walrus keeps the unreachable code after a `return_call`, here the only
;; use of its local, up to the `br`.
(module
  (func $f (result i32) (local i32)
    return_call $f
    local.get 0
    br 0
    i32.const 1
    drop))
