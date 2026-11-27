rockspec_format = "3.0"
package = "airsstack"
version = "0.0.1-1"
source = {
   url = "git+file:///private/tmp/claude-501/-Users-hiraq--herdr-worktrees-airsl-luarocks-integrations/4060dc68-5ed5-4b9d-947b-ba456a144f29/scratchpad/probes-M/toy",
   tag = "v0.0.1",
}
description = {
   summary = "probe M toy native module",
   license = "Apache-2.0",
}
dependencies = {
   "lua == 5.4",
}
build_dependencies = {
   "luarocks-build-rust-mlua == 0.2.7",
}
build = {
   type = "rust-mlua",
   cargo_extra_args = { "--locked" },
   modules = { airsstack = "airsstack" },
}
