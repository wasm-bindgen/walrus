(module
  (type (;0;) (func))
  (type (;1;) (func))
  (func $f (type 0))
  (func (;1;) (type 1))

  (export "a" (func $f))
  (export "b" (func 1))
  )

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (func $f (;0;) (type 0))
    (func (;1;) (type 0))
    (export "a" (func $f))
    (export "b" (func 1))
;)
