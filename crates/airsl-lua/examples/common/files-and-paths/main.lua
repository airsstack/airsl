-- Files, paths and globbing: `fs`, `path` and `glob` working together on a scratch directory.
--
-- A stock Lua script runs with the authority of the process, so there is no grant to ask for. The
-- scratch directory is still a fresh temporary one, removed at the end, so the example leaves
-- nothing behind and prints no path that belongs to one machine: every name below is relative.

local airsstack = require("airsstack")
local fs, path, glob = airsstack.fs, airsstack.path, airsstack.glob

local root = fs.tempdir()

-- `mkdir` creates missing parents too, and `path.join` spells the separator the same on every
-- platform.
fs.mkdir(path.join(root, "src", "util"))
fs.write(path.join(root, "README.md"), "# scratch\n")
fs.write(path.join(root, "src", "main.lua"), "print('main')\n")
fs.write(path.join(root, "src", "util", "strings.lua"), "return {}\n")
fs.write(path.join(root, "src", "util", "notes.txt"), "not code\n")

-- Reading back what was written.
local readme = fs.read(path.join(root, "README.md"))
print("README.md holds: " .. readme:gsub("\n$", ""))
print("src/main.lua is a file: " .. tostring(fs.is_file(path.join(root, "src", "main.lua"))))
print("src/util is a directory: " .. tostring(fs.is_dir(path.join(root, "src", "util"))))

-- Listings are sorted, so two machines print the same order.
print("top level: " .. table.concat(fs.list(root), ", "))
print("walk: " .. table.concat(fs.walk(root), ", "))

-- `*` stops at a `/`, `**/` matches any number of folders, including none.
print("lua files: " .. table.concat(glob.walk(root, "**/*.lua"), ", "))
print("lua files at the top: " .. #glob.walk(root, "*.lua"))
print("does src/util/strings.lua match **/*.lua: " .. tostring(glob.match("**/*.lua", "src/util/strings.lua")))

-- `path` is string arithmetic and never touches the disk.
local found = glob.walk(root, "**/strings.lua")[1]
print(found .. " -> " .. path.dirname(found) .. " + " .. path.basename(found))
print("stem and extension: " .. path.stem(found) .. " " .. path.ext(found))

-- `remove_dir` deletes the directory and everything under it.
fs.remove_dir(root)
print("scratch directory still exists: " .. tostring(fs.exists(root)))
