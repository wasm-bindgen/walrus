(module
  (memory 0)
  (func (;0;) (result i32)
    memory.size )
  (export "get" (func 0)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func (;0;) (type 0) (result i32)
      memory.size
    )
    (memory (;0;) 0)
    (export "get" (func 0))
;)
