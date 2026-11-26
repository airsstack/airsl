rockspec_format = "3.0"
package = "airsstack"
version = "0.0.1-1"
source = {
   url = "git+https://example.invalid/airsstack",
   tag = "v0.0.1",
}
description = {
   summary = "Toy probe of the airsstack native module",
   license = "MIT",
}
dependencies = {
   "lua == 5.4",
}
build_dependencies = {
   "luarocks-build-rust-mlua",
}
build = {
   type = "rust-mlua",
   cargo_extra_args = { "--package airsstack-native", "--locked" },
   modules = { airsstack = "airsstack" },
}
