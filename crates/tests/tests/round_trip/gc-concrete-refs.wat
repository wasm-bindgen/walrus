(module
  ;; GC types used as concrete ref types throughout the module
  (type $point (struct (field (mut f64)) (field (mut f64))))
  (type $iarr (array (mut i32)))
  (type $wrapper (struct (field i32) (field (ref null $point))))

  ;; Bug #2: Imported table with concrete element type
  (import "env" "struct_table" (table $st 5 (ref null $point)))

  ;; Bug #3: Imported global with concrete ref type
  (import "env" "ext_point" (global $ext_point (ref null $point)))

  ;; Bug #1: Local table with concrete element type
  (table $local_tbl 10 (ref null $wrapper))

  ;; Bug #4: Element segment with concrete ref type
  (elem $e1 (ref null $point) (ref.null $point) (ref.null $point))

  ;; Bug #5: ref.null with concrete type in const expr (global initializer)
  (global (export "null_point") (ref null $point) (ref.null $point))

  ;; Bug #6: ref.null with concrete type in function body
  (func (export "make_null_point") (result (ref null $point))
    ref.null $point
  )

  ;; Bug #7: typed select with concrete ref type in function body
  (func (export "select_point") (param (ref null $point) (ref null $point) i32) (result (ref null $point))
    local.get 0
    local.get 1
    local.get 2
    select (result (ref null $point))
  )

  ;; Bug #8: Function local with concrete ref type
  (func (export "use_local") (result (ref null $wrapper))
    (local $w (ref null $wrapper))
    local.get $w
  )

  ;; A function that uses the imported table and global to keep them alive
  (func (export "read_from_table") (param i32) (result (ref null $point))
    local.get 0
    table.get $st
  )

  (func (export "read_ext_point") (result (ref null $point))
    global.get $ext_point
  )

  ;; Use the local table to keep it alive
  (func (export "get_wrapper") (param i32) (result (ref null $wrapper))
    local.get 0
    table.get $local_tbl
  )
)

(; CHECK-ALL:
  (module
    (type $point (;0;) (struct (field (mut f64)) (field (mut f64))))
    (type $wrapper (;1;) (struct (field i32) (field (ref null 0))))
    (type (;2;) (func (result (ref null 0))))
    (type (;3;) (func (param (ref null 0) (ref null 0) i32) (result (ref null 0))))
    (type (;4;) (func (result (ref null 1))))
    (type (;5;) (func (param i32) (result (ref null 0))))
    (type (;6;) (func (param i32) (result (ref null 1))))
    (import "env" "struct_table" (table $st (;0;) 5 (ref null 0)))
    (import "env" "ext_point" (global $ext_point (;0;) (ref null 0)))
    (func (;0;) (type 3) (param (ref null 0) (ref null 0) i32) (result (ref null 0))
      local.get 0
      local.get 1
      local.get 2
      select (result (ref null 0))
    )
    (func (;1;) (type 5) (param i32) (result (ref null 0))
      local.get 0
      table.get $st
    )
    (func (;2;) (type 6) (param i32) (result (ref null 1))
      local.get 0
      table.get $local_tbl
    )
    (func (;3;) (type 2) (result (ref null 0))
      ref.null 0
    )
    (func (;4;) (type 4) (result (ref null 1))
      (local (ref null 1))
      local.get 0
    )
    (func (;5;) (type 2) (result (ref null 0))
      global.get $ext_point
    )
    (table $local_tbl (;1;) 10 (ref null 1))
    (global (;1;) (ref null 0) ref.null 0)
    (export "null_point" (global 1))
    (export "make_null_point" (func 3))
    (export "select_point" (func 0))
    (export "use_local" (func 4))
    (export "read_from_table" (func 1))
    (export "read_ext_point" (func 5))
    (export "get_wrapper" (func 2))
    (@producers
      (processed-by "walrus" "0.25.2")
    )
  )
;)
