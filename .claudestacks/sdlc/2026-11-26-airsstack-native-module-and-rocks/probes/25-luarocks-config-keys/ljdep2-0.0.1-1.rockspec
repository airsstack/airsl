
package = "ljdep2"
version = "0.0.1-1"
source = { url = "git+https://example.invalid/ljdep" }
description = { summary = "Probe: depends on the VM-provided luajit rock", license = "MIT" }
dependencies = { "luajit >= 2.1" }
build = { type = "none" }
