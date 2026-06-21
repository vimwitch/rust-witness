// This is a hack to avoid nesting packages
// relevant imports are stored in this file and
// imported unhygienically
include!("./src/transpile.rs");

// We only want to link our test witness functions if we're compiling
// for the tests in rust-witness. Cargo/rust doesn't seem to offer
// a solution so we use a simple env variable set using .cargo
fn main() {
    if std::env::var("RUST_WITNESS_LINK_TEST_WITNESS").is_ok() {
        transpile_wasm(String::from("./tests"));
    }
}
