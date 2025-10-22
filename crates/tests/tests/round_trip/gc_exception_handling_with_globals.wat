;; Keep tags and globals used in exception handling.
;; Function uses try_table with a tag, and the catch block uses a global.

(module
  ;; Tag for exceptions
  (tag $myTag (param i32))

  ;; Global used in catch handler
  (global $errorValue (mut i32) (i32.const 99))

  ;; Unused global that should be removed
  (global $unused i32 (i32.const 888))

  ;; Function that uses try_table with the tag and global
  (func $f (result i32)
    (block $catch (result i32)
      (try_table (result i32) (catch $myTag $catch)
        (i32.const 42)
      )
    )
    drop
    global.get $errorValue)

  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (type (;1;) (func (param i32)))
    (func $f (;0;) (type 0) (result i32)
      block (result i32) ;; label = @1
        try_table (result i32) (catch 0 0 (;@1;)) ;; label = @2
          i32.const 42
        end
      end
      drop
      global.get $errorValue
    )
    (tag (;0;) (type 1) (param i32))
    (global $errorValue (;0;) (mut i32) i32.const 99)
    (export "f" (func $f))
;)
