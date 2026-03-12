#![no_main]

#[macro_use]
extern crate libfuzzer_sys;

use walrus_fuzz_utils::{BufRng, Config, WasmOptTtf};

fuzz_target!(|data: &[u8]| {
    let data = if data.is_empty() { &[0] } else { data };
    let fuel = data.len();
    let rng = BufRng::new(data);
    let mut config = Config::<WasmOptTtf, BufRng>::new(rng).set_fuel(fuel);
    if let Err(e) = config.run_one() {
        walrus_fuzz_utils::print_err(&e);
        panic!("Found an error! {}", e);
    }
});
