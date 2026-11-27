-- Same, but points at the Lua 5.4 build (expected to fail: Neovim embeds LuaJIT).
package.cpath = arg[1] .. "/?.so;" .. package.cpath
local ok, err = pcall(require, "airsstack")
print("pcall ok=" .. tostring(ok), err)
