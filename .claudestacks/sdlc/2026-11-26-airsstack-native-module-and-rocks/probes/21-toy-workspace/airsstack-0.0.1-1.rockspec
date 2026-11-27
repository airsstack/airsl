rockspec_format = "3.0"
package = "airsstack"
version = "0.0.1-1"
source = {
   url = "git+file:///private/tmp/claude-501/-Users-hiraq--herdr-worktrees-airsl-luarocks-integrations/4060dc68-5ed5-4b9d-947b-ba456a144f29/scratchpad/probes-B/ws",
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
   cargo_extra_args = { "--package", "airsstack-native" },
   modules = { airsstack = "airsstack" },
}
