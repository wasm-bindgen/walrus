(module
 (memory 1)
 (data (i32.const 0))
 (export "" (func $b))
 (func $b
   data.drop 0))

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (func $b (;0;) (type 0)
      data.drop 0
    )
    (memory (;0;) 1)
    (export "" (func $b))
    (data (;0;) (i32.const 0) "")
;)