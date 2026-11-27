-- Loading the native module inside Neovim, which embeds LuaJIT, so the package is
-- `airsstack-luajit`.
--
-- Neovim does not search LuaRocks' folders, so the script asks LuaRocks where its C modules are
-- and adds them to `package.cpath`. It asks only when `require` cannot already find the module,
-- which is also what lets `LUA_CPATH` point it at a build that was never installed.
--
-- Output goes through `io.write`. Under `nvim -l`, `print` writes to standard error, and
-- `airsstack.stdio.write` is a different buffer from `io.write`, so the script keeps to one
-- channel and its lines stay in order.

if not pcall(require, "airsstack") then
  local cpath = vim.fn.system({ "luarocks", "--lua-version", "5.1", "path", "--lr-cpath" })
  assert(vim.v.shell_error == 0, cpath)
  package.cpath = package.cpath .. ";" .. vim.trim(cpath)
end

local airsstack = require("airsstack")

local function say(line)
  io.write(line, "\n")
end

say(airsstack.json.encode({ editor = "neovim", lua = _VERSION, jit = type(jit) == "table" }))
say("sha256: " .. airsstack.hash.sha256("abc"))
say("path: " .. airsstack.path.join("lua", "plugin", "init.lua"))
say("global airsstack: " .. tostring(rawget(_G, "airsstack")))
