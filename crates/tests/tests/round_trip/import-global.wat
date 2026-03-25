(module
  (import "" "" (global i32))
  (export "b" (global 0))
  )

(; CHECK-ALL:
  (module
    (import "" "" (global (;0;) i32))
    (export "b" (global 0))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
