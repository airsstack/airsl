rockspec_format = "3.0"
package = "airsstack"
version = "0.0.1-1"
source = {
   url = "git+https://example.invalid/airsstack",
   tag = "v0.0.1",
   dir = "crates/native",
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
   modules = {
      airsstack = "airsstack",
   },
}
