(module
  (func (export "i64.dup") (param i64) (result i64 i64)
    ;; With this commented out, there's only a single i64 on the stack.
    ;; get_local 0
    (local.get 0)))
