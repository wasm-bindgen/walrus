(module
  (type (;0;) (func))
  (func $f (type 0)
    (drop (i32.const 42)))
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (export "f" (func $f))
    (func $f (;0;) (type 0)
      i32.const 42
      drop
    )
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
