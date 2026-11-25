local function show(label, ok, msg) airsstack.stdio.write(label .. ": " .. tostring(ok) .. " " .. tostring(msg):gsub("\n.*", "") .. "\n") end
show("proc.run ls", pcall(airsstack.proc.run, {"ls", "-1"}))
show("proc.run ./nope", pcall(airsstack.proc.run, {"./nope"}))
show("fs.exists /etc/hosts", pcall(airsstack.fs.exists, "/etc/hosts"))
show("fs.exists /nonexistent", pcall(airsstack.fs.exists, "/nonexistent/x"))
