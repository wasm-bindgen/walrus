(module
  (type (;0;) (func (param i32) (result i32)))
  (func $inc (type 0) (param i32) (result i32)
    (i32.add
      (local.get 0)
      (i32.const 1)))
  (export "inc" (func $inc)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32) (result i32)))
    (func $inc (;0;) (type 0) (param i32) (result i32)
      local.get 0
      i32.const 1
      i32.add
    )
    (export "inc" (func $inc))
;)
