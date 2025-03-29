(module
  (import "x" "y" (func (param externref)))
  (func (export "a") (param externref)
    local.get 0
    call 0))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param externref)))
    (import "x" "y" (func (;0;) (type 0)))
    (func (;1;) (type 0) (param externref)
      local.get 0
      call 0
    )
    (export "a" (func 1))
;)