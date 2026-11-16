//! Source of the Rust fixtures; see `build.sh`.
#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

static mut COUNTER: u64 = 0;
static TABLE: [u16; 8] = [1, 1, 2, 3, 5, 8, 13, 21];

#[derive(Clone, Copy)]
pub struct Point {
    x: f64,
    y: f64,
    tag: u8,
}

#[inline(always)]
fn scale(p: Point, by: f64) -> Point {
    Point { x: p.x * by, y: p.y * by, tag: p.tag }
}

#[inline(never)]
fn norm2(p: Point) -> f64 {
    let s = scale(p, 2.0);
    s.x * s.x + s.y * s.y
}

#[inline(never)]
fn mix(a: u32, b: u64, c: f32) -> u64 {
    let mut acc = b;
    let mut i = 0;
    while i < a {
        let t = TABLE[(i % 8) as usize] as u64;
        if t % 2 == 0 {
            acc = acc.wrapping_mul(t + 1);
        } else {
            acc ^= (c as u64).rotate_left(i);
        }
        i += 1;
    }
    unsafe { COUNTER += acc & 0xff };
    acc
}

fn fib(n: u32) -> u32 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

#[no_mangle]
pub extern "C" fn entry(a: u32, b: u32) -> u64 {
    let p = Point { x: a as f64, y: b as f64, tag: 3 };
    let n = norm2(p);
    let m = mix(a, b as u64, n as f32);
    let f = (0..a).map(|i| fib(i % 10) as u64).sum::<u64>();
    m ^ f ^ unsafe { COUNTER }
}

#[no_mangle]
pub extern "C" fn select(which: i32, x: i64, y: i64) -> i64 {
    match which {
        0 => x + y,
        1 => x - y,
        2 => x.wrapping_mul(y),
        3 if y != 0 => x / y,
        _ => fib(which as u32) as i64,
    }
}
