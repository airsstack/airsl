//! Link arguments for the native module on macOS.
//!
//! A Lua module does not link a Lua library: the interpreter that loads it provides the C API.
//! macOS refuses an unresolved symbol at link time unless told to look it up at load time, so this
//! script passes that flag to the `cdylib` link, and only there. It is its own file because Cargo
//! runs a build script from nowhere else.
//!
//! No inline tests, per the unit-test-mandate exemption #5 (build script): it only prints link
//! arguments, and without them the macOS link of the `cdylib` fails outright, so the build that
//! precedes the interpreter check in `lua/check.lua` never finishes.
//!
//! Responsibilities: the link arguments of the `cdylib`.
//!
//! Non-responsibilities: choosing the Lua version, which the crate's Cargo features do.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-undefined");
        println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
    }
}
