(module
  (tag $e-i32-i32 (param i32 i32))

  (func $throw-1-2
    i32.const 1
    i32.const 2
    throw $e-i32-i32
  )

  (func (export "test-throw-1-2")
    (block $h (result i32 i32)
      (try_table (catch $e-i32-i32 $h)
        (call $throw-1-2)
      )
      (return)
    )
    (if (i32.ne (i32.const 2)) (then (unreachable)))
    (if (i32.ne (i32.const 1)) (then (unreachable)))
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (type (;1;) (func (result i32 i32)))
    (type (;2;) (func (param i32 i32)))
    (func (;0;) (type 0)
      block (type 1) (result i32 i32) ;; label = @1
        try_table (catch $e-i32-i32 0 (;@1;)) ;; label = @2
          call $throw-1-2
        end
        return
      end
      i32.const 2
      i32.ne
      if ;; label = @1
        unreachable
      else
      end
      i32.const 1
      i32.ne
      if ;; label = @1
        unreachable
      else
      end
    )
    (func $throw-1-2 (;1;) (type 0)
      i32.const 1
      i32.const 2
      throw $e-i32-i32
    )
    (tag $e-i32-i32 (;0;) (type 2) (param i32 i32))
    (export "test-throw-1-2" (func 0))
;)
