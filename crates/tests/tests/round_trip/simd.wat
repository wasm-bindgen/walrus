(module
  (memory 0)

  (func $v128.const (export "v128.const") (result v128)
    v128.const i32x4 1 2 3 4
  )

  (func $v128.load (export "v128.load") (param i32) (result v128)
    local.get 0
    v128.load
  )

  (func $v128.store (export "v128.store") (param i32) (param v128)
    local.get 0
    local.get 1
    v128.store
  )

(;
  (func $v128.shuffle (export "v128.shuffle") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    v8x16.shuffle 16 1 18 3 20 5 22 7 24 9 26 11 28 13 30 15)
;)

   (func $i8x16.splat (export "i8x16.splat") (param i32) (result v128)
     local.get 0
     i8x16.splat)
   (func $i8x16.extract_lane_s (export "i8x16.extract_lane_s") (param v128) (result i32)
     local.get 0
     i8x16.extract_lane_s 1)
   (func $i8x16.extract_lane_u (export "i8x16.extract_lane_u") (param v128) (result i32)
     local.get 0
     i8x16.extract_lane_u 2)
   (func $i8x16.replace_lane (export "i8x16.replace_lane") (param v128 i32) (result v128)
     local.get 0
     local.get 1
     i8x16.replace_lane 2)
   (func $i16x8.splat (export "i16x8.splat") (param i32) (result v128)
     local.get 0
     i16x8.splat)
   (func $i16x8.extract_lane_s (export "i16x8.extract_lane_s") (param v128) (result i32)
     local.get 0
     i16x8.extract_lane_s 1)
   (func $i16x8.extract_lane_u (export "i16x8.extract_lane_u") (param v128) (result i32)
     local.get 0
     i16x8.extract_lane_u 2)
   (func $i16x8.replace_lane (export "i16x8.replace_lane") (param v128 i32) (result v128)
     local.get 0
     local.get 1
     i16x8.replace_lane 2)
   (func $i32x4.splat (export "i32x4.splat") (param i32) (result v128)
     local.get 0
     i32x4.splat)
   (func $i32x4.extract_lane (export "i32x4.extract_lane") (param v128) (result i32)
     local.get 0
     i32x4.extract_lane 1)
   (func $i32x4.replace_lane (export "i32x4.replace_lane") (param v128 i32) (result v128)
     local.get 0
     local.get 1
     i32x4.replace_lane 2)
   (func $i64x2.splat (export "i64x2.splat") (param i64) (result v128)
     local.get 0
     i64x2.splat)
   (func $i64x2.extract_lane (export "i64x2.extract_lane") (param v128) (result i64)
     local.get 0
     i64x2.extract_lane 1)
   (func $i64x2.replace_lane (export "i64x2.replace_lane") (param v128 i64) (result v128)
     local.get 0
     local.get 1
     i64x2.replace_lane 0)
   (func $f32x4.splat (export "f32x4.splat") (param f32) (result v128)
     local.get 0
     f32x4.splat)
   (func $f32x4.extract_lane (export "f32x4.extract_lane") (param v128) (result f32)
     local.get 0
     f32x4.extract_lane 1)
   (func $f32x4.replace_lane (export "f32x4.replace_lane") (param v128 f32) (result v128)
     local.get 0
     local.get 1
     f32x4.replace_lane 2)
   (func $f64x2.splat (export "f64x2.splat") (param f64) (result v128)
     local.get 0
     f64x2.splat)
   (func $f64x2.extract_lane (export "f64x2.extract_lane") (param v128) (result f64)
     local.get 0
     f64x2.extract_lane 1)
   (func $f64x2.replace_lane (export "f64x2.replace_lane") (param v128 f64) (result v128)
     local.get 0
     local.get 1
     f64x2.replace_lane 0)

   (func $i8x16.eq (export "i8x16.eq") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.eq)
   (func $i8x16.lt_s (export "i8x16.lt_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.lt_s)
   (func $i8x16.lt_u (export "i8x16.lt_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.lt_u)
   (func $i8x16.gt_s (export "i8x16.gt_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.gt_s)
   (func $i8x16.gt_u (export "i8x16.gt_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.gt_u)
   (func $i8x16.le_s (export "i8x16.le_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.le_s)
   (func $i8x16.le_u (export "i8x16.le_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.le_u)
   (func $i8x16.ge_s (export "i8x16.ge_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.ge_s)
   (func $i8x16.ge_u (export "i8x16.ge_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.ge_u)

   (func $i16x8.eq (export "i16x8.eq") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.eq)
   (func $i16x8.lt_s (export "i16x8.lt_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.lt_s)
   (func $i16x8.lt_u (export "i16x8.lt_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.lt_u)
   (func $i16x8.gt_s (export "i16x8.gt_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.gt_s)
   (func $i16x8.gt_u (export "i16x8.gt_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.gt_u)
   (func $i16x8.le_s (export "i16x8.le_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.le_s)
   (func $i16x8.le_u (export "i16x8.le_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.le_u)
   (func $i16x8.ge_s (export "i16x8.ge_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.ge_s)
   (func $i16x8.ge_u (export "i16x8.ge_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.ge_u)

   (func $i32x4.eq (export "i32x4.eq") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.eq)
   (func $i32x4.lt_s (export "i32x4.lt_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.lt_s)
   (func $i32x4.lt_u (export "i32x4.lt_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.lt_u)
   (func $i32x4.gt_s (export "i32x4.gt_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.gt_s)
   (func $i32x4.gt_u (export "i32x4.gt_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.gt_u)
   (func $i32x4.le_s (export "i32x4.le_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.le_s)
   (func $i32x4.le_u (export "i32x4.le_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.le_u)
   (func $i32x4.ge_s (export "i32x4.ge_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.ge_s)
   (func $i32x4.ge_u (export "i32x4.ge_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.ge_u)

   (func $f32x4.eq (export "f32x4.eq") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.eq)
   (func $f32x4.lt (export "f32x4.lt") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.lt)
   (func $f32x4.gt (export "f32x4.gt") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.gt)
   (func $f32x4.le (export "f32x4.le") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.le)
   (func $f32x4.ge (export "f32x4.ge") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.ge)

   (func $f64x2.eq (export "f64x2.eq") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.eq)
   (func $f64x2.lt (export "f64x2.lt") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.lt)
   (func $f64x2.gt (export "f64x2.gt") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.gt)
   (func $f64x2.le (export "f64x2.le") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.le)
   (func $f64x2.ge (export "f64x2.ge") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.ge)

   (func $v128.not (export "v128.not") (param v128) (result v128)
    local.get 0
    v128.not)
   (func $v128.and (export "v128.and") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    v128.and)
   (func $v128.or (export "v128.or") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    v128.or)
   (func $v128.xor (export "v128.xor") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    v128.xor)
   (func $v128.bitselect (export "v128.bitselect") (param v128 v128 v128) (result v128)
    local.get 0
    local.get 1
    local.get 2
    v128.bitselect)
   (func $v128.any_true (export "v128.any_true") (param v128) (result i32)
    local.get 0
    v128.any_true)

   (func $i8x16.neg (export "i8x16.neg") (param v128) (result v128)
    local.get 0
    i8x16.neg)
   (func $i8x16.all_true (export "i8x16.all_true") (param v128) (result i32)
    local.get 0
    i8x16.all_true)
   (func $i8x16.shl (export "i8x16.shl") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i8x16.shl)
   (func $i8x16.shr_s (export "i8x16.shr_s") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i8x16.shr_s)
   (func $i8x16.shr_u (export "i8x16.shr_u") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i8x16.shr_u)
   (func $i8x16.add (export "i8x16.add") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.add)
   (func $i8x16.add_sat_u (export "i8x16.add_sat_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.add_sat_u)
   (func $i8x16.add_sat_s (export "i8x16.add_sat_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.add_sat_s)
   (func $i8x16.sub (export "i8x16.sub") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.sub)
   (func $i8x16.sub_sat_u (export "i8x16.sub_sat_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.sub_sat_u)
   (func $i8x16.sub_sat_s (export "i8x16.sub_sat_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i8x16.sub_sat_s)

   (func $i16x8.neg (export "i16x8.neg") (param v128) (result v128)
    local.get 0
    i16x8.neg)
   (func $i16x8.all_true (export "i16x8.all_true") (param v128) (result i32)
    local.get 0
    i16x8.all_true)
   (func $i16x8.shl (export "i16x8.shl") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i16x8.shl)
   (func $i16x8.shr_s (export "i16x8.shr_s") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i16x8.shr_s)
   (func $i16x8.shr_u (export "i16x8.shr_u") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i16x8.shr_u)
   (func $i16x8.add (export "i16x8.add") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.add)
   (func $i16x8.add_sat_u (export "i16x8.add_sat_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.add_sat_u)
   (func $i16x8.add_sat_s (export "i16x8.add_sat_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.add_sat_s)
   (func $i16x8.sub (export "i16x8.sub") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.sub)
   (func $i16x8.sub_sat_u (export "i16x8.sub_sat_u") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.sub_sat_u)
   (func $i16x8.sub_sat_s (export "i16x8.sub_sat_s") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.sub_sat_s)
   (func $i16x8.mul (export "i16x8.mul") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i16x8.mul)

   (func $i32x4.neg (export "i32x4.neg") (param v128) (result v128)
    local.get 0
    i32x4.neg)
   (func $i32x4.all_true (export "i32x4.all_true") (param v128) (result i32)
    local.get 0
    i32x4.all_true)
   (func $i32x4.shl (export "i32x4.shl") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i32x4.shl)
   (func $i32x4.shr_s (export "i32x4.shr_s") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i32x4.shr_s)
   (func $i32x4.shr_u (export "i32x4.shr_u") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i32x4.shr_u)
   (func $i32x4.add (export "i32x4.add") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.add)
   (func $i32x4.sub (export "i32x4.sub") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.sub)
   (func $i32x4.mul (export "i32x4.mul") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i32x4.mul)

   (func $i64x2.neg (export "i64x2.neg") (param v128) (result v128)
    local.get 0
    i64x2.neg)
   (func $i64x2.shl (export "i64x2.shl") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i64x2.shl)
   (func $i64x2.shr_s (export "i64x2.shr_s") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i64x2.shr_s)
   (func $i64x2.shr_u (export "i64x2.shr_u") (param v128 i32) (result v128)
    local.get 0
    local.get 1
    i64x2.shr_u)
   (func $i64x2.add (export "i64x2.add") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i64x2.add)
   (func $i64x2.sub (export "i64x2.sub") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    i64x2.sub)

   (func $f32x4.abs (export "f32x4.abs") (param v128) (result v128)
    local.get 0
    f32x4.abs)
   (func $f32x4.neg (export "f32x4.neg") (param v128) (result v128)
    local.get 0
    f32x4.neg)
   (func $f32x4.sqrt (export "f32x4.sqrt") (param v128) (result v128)
    local.get 0
    f32x4.sqrt)
   (func $f32x4.add (export "f32x4.add") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.add)
   (func $f32x4.sub (export "f32x4.sub") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.sub)
   (func $f32x4.mul (export "f32x4.mul") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.mul)
   (func $f32x4.div (export "f32x4.div") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.div)
   (func $f32x4.min (export "f32x4.min") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.min)
   (func $f32x4.max (export "f32x4.max") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f32x4.max)

   (func $f64x2.abs (export "f64x2.abs") (param v128) (result v128)
    local.get 0
    f64x2.abs)
   (func $f64x2.neg (export "f64x2.neg") (param v128) (result v128)
    local.get 0
    f64x2.neg)
   (func $f64x2.sqrt (export "f64x2.sqrt") (param v128) (result v128)
    local.get 0
    f64x2.sqrt)
   (func $f64x2.add (export "f64x2.add") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.add)
   (func $f64x2.sub (export "f64x2.sub") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.sub)
   (func $f64x2.mul (export "f64x2.mul") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.mul)
   (func $f64x2.div (export "f64x2.div") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.div)
   (func $f64x2.min (export "f64x2.min") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.min)
   (func $f64x2.max (export "f64x2.max") (param v128 v128) (result v128)
    local.get 0
    local.get 1
    f64x2.max)

   (func $i32x4_trunc_s_f32x4_sat (export "i32x4_trunc_s_f32x4_sat") (param v128) (result v128)
    local.get 0
    i32x4.trunc_sat_f32x4_s)
   (func $i32x4_trunc_u_f32x4_sat (export "i32x4_trunc_u_f32x4_sat") (param v128) (result v128)
    local.get 0
    i32x4.trunc_sat_f32x4_u)

   (func $f32x4.convert_i32x4_s (export "f32x4.convert_i32x4_s") (param v128) (result v128)
    local.get 0
    f32x4.convert_i32x4_s)
   (func $f32x4.convert_i32x4_u (export "f32x4.convert_i32x4_u") (param v128) (result v128)
    local.get 0
    f32x4.convert_i32x4_u)
)

(; CHECK-ALL:
  (module
    (type (;0;) (func (result v128)))
    (type (;1;) (func (param i32) (result v128)))
    (type (;2;) (func (param i32 v128)))
    (type (;3;) (func (param i64) (result v128)))
    (type (;4;) (func (param f32) (result v128)))
    (type (;5;) (func (param f64) (result v128)))
    (type (;6;) (func (param v128) (result i32)))
    (type (;7;) (func (param v128) (result i64)))
    (type (;8;) (func (param v128) (result f32)))
    (type (;9;) (func (param v128) (result f64)))
    (type (;10;) (func (param v128) (result v128)))
    (type (;11;) (func (param v128 i32) (result v128)))
    (type (;12;) (func (param v128 i64) (result v128)))
    (type (;13;) (func (param v128 f32) (result v128)))
    (type (;14;) (func (param v128 f64) (result v128)))
    (type (;15;) (func (param v128 v128) (result v128)))
    (type (;16;) (func (param v128 v128 v128) (result v128)))
    (func $v128.bitselect (;0;) (type 16) (param v128 v128 v128) (result v128)
      local.get 0
      local.get 1
      local.get 2
      v128.bitselect
    )
    (func $v128.store (;1;) (type 2) (param i32 v128)
      local.get 0
      local.get 1
      v128.store
    )
    (func $i8x16.replace_lane (;2;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i8x16.replace_lane 2
    )
    (func $i16x8.replace_lane (;3;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i16x8.replace_lane 2
    )
    (func $i32x4.replace_lane (;4;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i32x4.replace_lane 2
    )
    (func $i64x2.replace_lane (;5;) (type 12) (param v128 i64) (result v128)
      local.get 0
      local.get 1
      i64x2.replace_lane 0
    )
    (func $f32x4.replace_lane (;6;) (type 13) (param v128 f32) (result v128)
      local.get 0
      local.get 1
      f32x4.replace_lane 2
    )
    (func $f64x2.replace_lane (;7;) (type 14) (param v128 f64) (result v128)
      local.get 0
      local.get 1
      f64x2.replace_lane 0
    )
    (func $i8x16.eq (;8;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.eq
    )
    (func $i8x16.lt_s (;9;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.lt_s
    )
    (func $i8x16.lt_u (;10;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.lt_u
    )
    (func $i8x16.gt_s (;11;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.gt_s
    )
    (func $i8x16.gt_u (;12;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.gt_u
    )
    (func $i8x16.le_s (;13;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.le_s
    )
    (func $i8x16.le_u (;14;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.le_u
    )
    (func $i8x16.ge_s (;15;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.ge_s
    )
    (func $i8x16.ge_u (;16;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.ge_u
    )
    (func $i16x8.eq (;17;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.eq
    )
    (func $i16x8.lt_s (;18;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.lt_s
    )
    (func $i16x8.lt_u (;19;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.lt_u
    )
    (func $i16x8.gt_s (;20;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.gt_s
    )
    (func $i16x8.gt_u (;21;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.gt_u
    )
    (func $i16x8.le_s (;22;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.le_s
    )
    (func $i16x8.le_u (;23;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.le_u
    )
    (func $i16x8.ge_s (;24;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.ge_s
    )
    (func $i16x8.ge_u (;25;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.ge_u
    )
    (func $i32x4.eq (;26;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.eq
    )
    (func $i32x4.lt_s (;27;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.lt_s
    )
    (func $i32x4.lt_u (;28;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.lt_u
    )
    (func $i32x4.gt_s (;29;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.gt_s
    )
    (func $i32x4.gt_u (;30;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.gt_u
    )
    (func $i32x4.le_s (;31;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.le_s
    )
    (func $i32x4.le_u (;32;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.le_u
    )
    (func $i32x4.ge_s (;33;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.ge_s
    )
    (func $i32x4.ge_u (;34;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.ge_u
    )
    (func $f32x4.eq (;35;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.eq
    )
    (func $f32x4.lt (;36;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.lt
    )
    (func $f32x4.gt (;37;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.gt
    )
    (func $f32x4.le (;38;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.le
    )
    (func $f32x4.ge (;39;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.ge
    )
    (func $f64x2.eq (;40;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.eq
    )
    (func $f64x2.lt (;41;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.lt
    )
    (func $f64x2.gt (;42;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.gt
    )
    (func $f64x2.le (;43;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.le
    )
    (func $f64x2.ge (;44;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.ge
    )
    (func $v128.and (;45;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      v128.and
    )
    (func $v128.or (;46;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      v128.or
    )
    (func $v128.xor (;47;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      v128.xor
    )
    (func $i8x16.shl (;48;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i8x16.shl
    )
    (func $i8x16.shr_s (;49;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i8x16.shr_s
    )
    (func $i8x16.shr_u (;50;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i8x16.shr_u
    )
    (func $i8x16.add (;51;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.add
    )
    (func $i8x16.add_sat_u (;52;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.add_sat_u
    )
    (func $i8x16.add_sat_s (;53;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.add_sat_s
    )
    (func $i8x16.sub (;54;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.sub
    )
    (func $i8x16.sub_sat_u (;55;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.sub_sat_u
    )
    (func $i8x16.sub_sat_s (;56;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i8x16.sub_sat_s
    )
    (func $i16x8.shl (;57;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i16x8.shl
    )
    (func $i16x8.shr_s (;58;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i16x8.shr_s
    )
    (func $i16x8.shr_u (;59;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i16x8.shr_u
    )
    (func $i16x8.add (;60;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.add
    )
    (func $i16x8.add_sat_u (;61;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.add_sat_u
    )
    (func $i16x8.add_sat_s (;62;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.add_sat_s
    )
    (func $i16x8.sub (;63;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.sub
    )
    (func $i16x8.sub_sat_u (;64;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.sub_sat_u
    )
    (func $i16x8.sub_sat_s (;65;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.sub_sat_s
    )
    (func $i16x8.mul (;66;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i16x8.mul
    )
    (func $i32x4.shl (;67;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i32x4.shl
    )
    (func $i32x4.shr_s (;68;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i32x4.shr_s
    )
    (func $i32x4.shr_u (;69;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i32x4.shr_u
    )
    (func $i32x4.add (;70;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.add
    )
    (func $i32x4.sub (;71;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.sub
    )
    (func $i32x4.mul (;72;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i32x4.mul
    )
    (func $i64x2.shl (;73;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i64x2.shl
    )
    (func $i64x2.shr_s (;74;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i64x2.shr_s
    )
    (func $i64x2.shr_u (;75;) (type 11) (param v128 i32) (result v128)
      local.get 0
      local.get 1
      i64x2.shr_u
    )
    (func $i64x2.add (;76;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i64x2.add
    )
    (func $i64x2.sub (;77;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      i64x2.sub
    )
    (func $f32x4.add (;78;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.add
    )
    (func $f32x4.sub (;79;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.sub
    )
    (func $f32x4.mul (;80;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.mul
    )
    (func $f32x4.div (;81;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.div
    )
    (func $f32x4.min (;82;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.min
    )
    (func $f32x4.max (;83;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f32x4.max
    )
    (func $f64x2.add (;84;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.add
    )
    (func $f64x2.sub (;85;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.sub
    )
    (func $f64x2.mul (;86;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.mul
    )
    (func $f64x2.div (;87;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.div
    )
    (func $f64x2.min (;88;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.min
    )
    (func $f64x2.max (;89;) (type 15) (param v128 v128) (result v128)
      local.get 0
      local.get 1
      f64x2.max
    )
    (func $v128.load (;90;) (type 1) (param i32) (result v128)
      local.get 0
      v128.load
    )
    (func $i8x16.splat (;91;) (type 1) (param i32) (result v128)
      local.get 0
      i8x16.splat
    )
    (func $i8x16.extract_lane_s (;92;) (type 6) (param v128) (result i32)
      local.get 0
      i8x16.extract_lane_s 1
    )
    (func $i8x16.extract_lane_u (;93;) (type 6) (param v128) (result i32)
      local.get 0
      i8x16.extract_lane_u 2
    )
    (func $i16x8.splat (;94;) (type 1) (param i32) (result v128)
      local.get 0
      i16x8.splat
    )
    (func $i16x8.extract_lane_s (;95;) (type 6) (param v128) (result i32)
      local.get 0
      i16x8.extract_lane_s 1
    )
    (func $i16x8.extract_lane_u (;96;) (type 6) (param v128) (result i32)
      local.get 0
      i16x8.extract_lane_u 2
    )
    (func $i32x4.splat (;97;) (type 1) (param i32) (result v128)
      local.get 0
      i32x4.splat
    )
    (func $i32x4.extract_lane (;98;) (type 6) (param v128) (result i32)
      local.get 0
      i32x4.extract_lane 1
    )
    (func $i64x2.splat (;99;) (type 3) (param i64) (result v128)
      local.get 0
      i64x2.splat
    )
    (func $i64x2.extract_lane (;100;) (type 7) (param v128) (result i64)
      local.get 0
      i64x2.extract_lane 1
    )
    (func $f32x4.splat (;101;) (type 4) (param f32) (result v128)
      local.get 0
      f32x4.splat
    )
    (func $f32x4.extract_lane (;102;) (type 8) (param v128) (result f32)
      local.get 0
      f32x4.extract_lane 1
    )
    (func $f64x2.splat (;103;) (type 5) (param f64) (result v128)
      local.get 0
      f64x2.splat
    )
    (func $f64x2.extract_lane (;104;) (type 9) (param v128) (result f64)
      local.get 0
      f64x2.extract_lane 1
    )
    (func $v128.not (;105;) (type 10) (param v128) (result v128)
      local.get 0
      v128.not
    )
    (func $v128.any_true (;106;) (type 6) (param v128) (result i32)
      local.get 0
      v128.any_true
    )
    (func $i8x16.neg (;107;) (type 10) (param v128) (result v128)
      local.get 0
      i8x16.neg
    )
    (func $i8x16.all_true (;108;) (type 6) (param v128) (result i32)
      local.get 0
      i8x16.all_true
    )
    (func $i16x8.neg (;109;) (type 10) (param v128) (result v128)
      local.get 0
      i16x8.neg
    )
    (func $i16x8.all_true (;110;) (type 6) (param v128) (result i32)
      local.get 0
      i16x8.all_true
    )
    (func $i32x4.neg (;111;) (type 10) (param v128) (result v128)
      local.get 0
      i32x4.neg
    )
    (func $i32x4.all_true (;112;) (type 6) (param v128) (result i32)
      local.get 0
      i32x4.all_true
    )
    (func $i64x2.neg (;113;) (type 10) (param v128) (result v128)
      local.get 0
      i64x2.neg
    )
    (func $f32x4.abs (;114;) (type 10) (param v128) (result v128)
      local.get 0
      f32x4.abs
    )
    (func $f32x4.neg (;115;) (type 10) (param v128) (result v128)
      local.get 0
      f32x4.neg
    )
    (func $f32x4.sqrt (;116;) (type 10) (param v128) (result v128)
      local.get 0
      f32x4.sqrt
    )
    (func $f64x2.abs (;117;) (type 10) (param v128) (result v128)
      local.get 0
      f64x2.abs
    )
    (func $f64x2.neg (;118;) (type 10) (param v128) (result v128)
      local.get 0
      f64x2.neg
    )
    (func $f64x2.sqrt (;119;) (type 10) (param v128) (result v128)
      local.get 0
      f64x2.sqrt
    )
    (func $i32x4_trunc_s_f32x4_sat (;120;) (type 10) (param v128) (result v128)
      local.get 0
      i32x4.trunc_sat_f32x4_s
    )
    (func $i32x4_trunc_u_f32x4_sat (;121;) (type 10) (param v128) (result v128)
      local.get 0
      i32x4.trunc_sat_f32x4_u
    )
    (func $f32x4.convert_i32x4_s (;122;) (type 10) (param v128) (result v128)
      local.get 0
      f32x4.convert_i32x4_s
    )
    (func $f32x4.convert_i32x4_u (;123;) (type 10) (param v128) (result v128)
      local.get 0
      f32x4.convert_i32x4_u
    )
    (func $v128.const (;124;) (type 0) (result v128)
      v128.const i32x4 0x00000001 0x00000002 0x00000003 0x00000004
    )
    (memory (;0;) 0)
    (export "v128.const" (func $v128.const))
    (export "v128.load" (func $v128.load))
    (export "v128.store" (func $v128.store))
    (export "i8x16.splat" (func $i8x16.splat))
    (export "i8x16.extract_lane_s" (func $i8x16.extract_lane_s))
    (export "i8x16.extract_lane_u" (func $i8x16.extract_lane_u))
    (export "i8x16.replace_lane" (func $i8x16.replace_lane))
    (export "i16x8.splat" (func $i16x8.splat))
    (export "i16x8.extract_lane_s" (func $i16x8.extract_lane_s))
    (export "i16x8.extract_lane_u" (func $i16x8.extract_lane_u))
    (export "i16x8.replace_lane" (func $i16x8.replace_lane))
    (export "i32x4.splat" (func $i32x4.splat))
    (export "i32x4.extract_lane" (func $i32x4.extract_lane))
    (export "i32x4.replace_lane" (func $i32x4.replace_lane))
    (export "i64x2.splat" (func $i64x2.splat))
    (export "i64x2.extract_lane" (func $i64x2.extract_lane))
    (export "i64x2.replace_lane" (func $i64x2.replace_lane))
    (export "f32x4.splat" (func $f32x4.splat))
    (export "f32x4.extract_lane" (func $f32x4.extract_lane))
    (export "f32x4.replace_lane" (func $f32x4.replace_lane))
    (export "f64x2.splat" (func $f64x2.splat))
    (export "f64x2.extract_lane" (func $f64x2.extract_lane))
    (export "f64x2.replace_lane" (func $f64x2.replace_lane))
    (export "i8x16.eq" (func $i8x16.eq))
    (export "i8x16.lt_s" (func $i8x16.lt_s))
    (export "i8x16.lt_u" (func $i8x16.lt_u))
    (export "i8x16.gt_s" (func $i8x16.gt_s))
    (export "i8x16.gt_u" (func $i8x16.gt_u))
    (export "i8x16.le_s" (func $i8x16.le_s))
    (export "i8x16.le_u" (func $i8x16.le_u))
    (export "i8x16.ge_s" (func $i8x16.ge_s))
    (export "i8x16.ge_u" (func $i8x16.ge_u))
    (export "i16x8.eq" (func $i16x8.eq))
    (export "i16x8.lt_s" (func $i16x8.lt_s))
    (export "i16x8.lt_u" (func $i16x8.lt_u))
    (export "i16x8.gt_s" (func $i16x8.gt_s))
    (export "i16x8.gt_u" (func $i16x8.gt_u))
    (export "i16x8.le_s" (func $i16x8.le_s))
    (export "i16x8.le_u" (func $i16x8.le_u))
    (export "i16x8.ge_s" (func $i16x8.ge_s))
    (export "i16x8.ge_u" (func $i16x8.ge_u))
    (export "i32x4.eq" (func $i32x4.eq))
    (export "i32x4.lt_s" (func $i32x4.lt_s))
    (export "i32x4.lt_u" (func $i32x4.lt_u))
    (export "i32x4.gt_s" (func $i32x4.gt_s))
    (export "i32x4.gt_u" (func $i32x4.gt_u))
    (export "i32x4.le_s" (func $i32x4.le_s))
    (export "i32x4.le_u" (func $i32x4.le_u))
    (export "i32x4.ge_s" (func $i32x4.ge_s))
    (export "i32x4.ge_u" (func $i32x4.ge_u))
    (export "f32x4.eq" (func $f32x4.eq))
    (export "f32x4.lt" (func $f32x4.lt))
    (export "f32x4.gt" (func $f32x4.gt))
    (export "f32x4.le" (func $f32x4.le))
    (export "f32x4.ge" (func $f32x4.ge))
    (export "f64x2.eq" (func $f64x2.eq))
    (export "f64x2.lt" (func $f64x2.lt))
    (export "f64x2.gt" (func $f64x2.gt))
    (export "f64x2.le" (func $f64x2.le))
    (export "f64x2.ge" (func $f64x2.ge))
    (export "v128.not" (func $v128.not))
    (export "v128.and" (func $v128.and))
    (export "v128.or" (func $v128.or))
    (export "v128.xor" (func $v128.xor))
    (export "v128.bitselect" (func $v128.bitselect))
    (export "v128.any_true" (func $v128.any_true))
    (export "i8x16.neg" (func $i8x16.neg))
    (export "i8x16.all_true" (func $i8x16.all_true))
    (export "i8x16.shl" (func $i8x16.shl))
    (export "i8x16.shr_s" (func $i8x16.shr_s))
    (export "i8x16.shr_u" (func $i8x16.shr_u))
    (export "i8x16.add" (func $i8x16.add))
    (export "i8x16.add_sat_u" (func $i8x16.add_sat_u))
    (export "i8x16.add_sat_s" (func $i8x16.add_sat_s))
    (export "i8x16.sub" (func $i8x16.sub))
    (export "i8x16.sub_sat_u" (func $i8x16.sub_sat_u))
    (export "i8x16.sub_sat_s" (func $i8x16.sub_sat_s))
    (export "i16x8.neg" (func $i16x8.neg))
    (export "i16x8.all_true" (func $i16x8.all_true))
    (export "i16x8.shl" (func $i16x8.shl))
    (export "i16x8.shr_s" (func $i16x8.shr_s))
    (export "i16x8.shr_u" (func $i16x8.shr_u))
    (export "i16x8.add" (func $i16x8.add))
    (export "i16x8.add_sat_u" (func $i16x8.add_sat_u))
    (export "i16x8.add_sat_s" (func $i16x8.add_sat_s))
    (export "i16x8.sub" (func $i16x8.sub))
    (export "i16x8.sub_sat_u" (func $i16x8.sub_sat_u))
    (export "i16x8.sub_sat_s" (func $i16x8.sub_sat_s))
    (export "i16x8.mul" (func $i16x8.mul))
    (export "i32x4.neg" (func $i32x4.neg))
    (export "i32x4.all_true" (func $i32x4.all_true))
    (export "i32x4.shl" (func $i32x4.shl))
    (export "i32x4.shr_s" (func $i32x4.shr_s))
    (export "i32x4.shr_u" (func $i32x4.shr_u))
    (export "i32x4.add" (func $i32x4.add))
    (export "i32x4.sub" (func $i32x4.sub))
    (export "i32x4.mul" (func $i32x4.mul))
    (export "i64x2.neg" (func $i64x2.neg))
    (export "i64x2.shl" (func $i64x2.shl))
    (export "i64x2.shr_s" (func $i64x2.shr_s))
    (export "i64x2.shr_u" (func $i64x2.shr_u))
    (export "i64x2.add" (func $i64x2.add))
    (export "i64x2.sub" (func $i64x2.sub))
    (export "f32x4.abs" (func $f32x4.abs))
    (export "f32x4.neg" (func $f32x4.neg))
    (export "f32x4.sqrt" (func $f32x4.sqrt))
    (export "f32x4.add" (func $f32x4.add))
    (export "f32x4.sub" (func $f32x4.sub))
    (export "f32x4.mul" (func $f32x4.mul))
    (export "f32x4.div" (func $f32x4.div))
    (export "f32x4.min" (func $f32x4.min))
    (export "f32x4.max" (func $f32x4.max))
    (export "f64x2.abs" (func $f64x2.abs))
    (export "f64x2.neg" (func $f64x2.neg))
    (export "f64x2.sqrt" (func $f64x2.sqrt))
    (export "f64x2.add" (func $f64x2.add))
    (export "f64x2.sub" (func $f64x2.sub))
    (export "f64x2.mul" (func $f64x2.mul))
    (export "f64x2.div" (func $f64x2.div))
    (export "f64x2.min" (func $f64x2.min))
    (export "f64x2.max" (func $f64x2.max))
    (export "i32x4_trunc_s_f32x4_sat" (func $i32x4_trunc_s_f32x4_sat))
    (export "i32x4_trunc_u_f32x4_sat" (func $i32x4_trunc_u_f32x4_sat))
    (export "f32x4.convert_i32x4_s" (func $f32x4.convert_i32x4_s))
    (export "f32x4.convert_i32x4_u" (func $f32x4.convert_i32x4_u))
;)
