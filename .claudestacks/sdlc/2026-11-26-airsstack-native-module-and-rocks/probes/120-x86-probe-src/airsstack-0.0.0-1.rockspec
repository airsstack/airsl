rockspec_format = "3.0"
package = "airsstack"
version = "0.0.0-1"
source = { url = "git+https://github.com/airsstack/airsl" }
description = { summary = "X probe toy rock", license = "MIT" }
dependencies = { "lua >= 5.1" }
build_dependencies = { "luarocks-build-rust-mlua" }
build = { type = "rust-mlua", modules = { "airsstack" } }
