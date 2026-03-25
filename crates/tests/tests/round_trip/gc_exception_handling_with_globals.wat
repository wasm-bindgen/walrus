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

;; CHECK: (module
;; NEXT: (type (;0;) (func (param i32)))
;; NEXT: (type (;1;) (func (result i32)))
;; NEXT: (func $f (;0;) (type 1) (result i32)
;; NEXT: block (result i32) ;; label = @1
;; NEXT: try_table (result i32) (catch $myTag 0 (;@1;)) ;; label = @2
;; NEXT: i32.const 42
;; NEXT: end
;; NEXT: end
;; NEXT: drop
;; NEXT: global.get $errorValue
;; NEXT: )
;; NEXT: (tag $myTag (;0;) (type 0) (param i32))
;; NEXT: (global $errorValue (;0;) (mut i32) i32.const 99)
;; NEXT: (export "f" (func $f))
