;; Defined functions are numbered by how often their index is written, most
;; first, so the shortest LEB128 indices go to the most-referenced functions.
;; Functions referenced equally often are ordered largest first.
(module
  (table (export "table") 2 funcref)
  (elem (i32.const 0) func $in_table $in_table)
  (func $big (export "big") (param i32) (result i32)
    local.get 0
    call $called_three_times
    local.get 0
    call $called_three_times
    i32.add
    local.get 0
    call $called_three_times
    i32.add
    call $called_once
    i32.add)
  (func $called_once (result i32)
    i32.const 1)
  (func $called_three_times (param i32) (result i32)
    local.get 0
    i32.const 3
    i32.mul)
  (func $in_table (result i32)
    i32.const 2)
  (func $tail (export "tail") (param i32) (result i32)
    local.get 0
    return_call $called_three_times))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (type (;1;) (func (param i32) (result i32)))
    (table (;0;) 2 funcref)
    (export "table" (table 0))
    (export "big" (func $big))
    (export "tail" (func $tail))
    (elem (;0;) (i32.const 0) func $in_table $in_table)
    (func $called_three_times (;0;) (type 1) (param i32) (result i32)
      local.get 0
      i32.const 3
      i32.mul
    )
    (func $in_table (;1;) (type 0) (result i32)
      i32.const 2
    )
    (func $big (;2;) (type 1) (param i32) (result i32)
      local.get 0
      call $called_three_times
      local.get 0
      call $called_three_times
      i32.add
      local.get 0
      call $called_three_times
      i32.add
      call $called_once
      i32.add
    )
    (func $tail (;3;) (type 1) (param i32) (result i32)
      local.get 0
      return_call $called_three_times
    )
    (func $called_once (;4;) (type 0) (result i32)
      i32.const 1
    )
  )
;)
