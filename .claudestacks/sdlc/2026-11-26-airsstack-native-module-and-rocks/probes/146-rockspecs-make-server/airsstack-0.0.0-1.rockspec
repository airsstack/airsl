rockspec_format = "3.0"
package = "airsstack"
version = "0.0.0-1"
source = {
   url = "git+file:///private/tmp/claude-501/-Users-hiraq--herdr-worktrees-airsl-luarocks-integrations/4060dc68-5ed5-4b9d-947b-ba456a144f29/scratchpad/probes-L/repo",
   tag = "airsl-cli-v0.0.0",
}
description = {
   summary = "airsl's host standard library as a native Lua module (probe L)",
   homepage = "https://airsstack.github.io/airsl/",
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
   cargo_extra_args = { "--package", "airsl-lua", "--locked" },
   modules = { airsstack = "airsstack" },
}
