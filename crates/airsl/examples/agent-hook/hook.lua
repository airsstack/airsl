-- A PreToolUse hook, end to end: take the payload, decide, write the envelope.
--
-- The host passes this directory as an argument, so nothing here is spelled as a path that is
-- true only on the machine the example was built on.

local here = arg[1]

-- The stdin dance below is scaffolding, and it is the one place this example is not what you
-- would ship. A real hook's first line is `local payload = airsstack.hook.payload()` and there is
-- no second one, because the host always has a document to hand it.
--
-- `payload()` reads *this process's own* stdin, and there is no seam to inject anything else
-- through -- so read unconditionally and the example blocks forever on a terminal, waiting for a
-- person to type JSON. Asking `isatty` first is what avoids that: a terminal means nobody piped
-- anything in, so there is nothing to read. Under `cargo make examples` stdin is not a tty but is
-- already at end of stream, and `payload()` answers an empty table rather than raising, because
-- no payload is not a parse failure (src/modules/hook.rs:90).
--
-- Both of those fall through to the bundled `payload.json`, so the terminal run and the CI run
-- print the same bytes -- and piping that same file in takes the `payload()` path and prints them
-- a third time, because it is the same document.
local payload = {}
if not airsstack.stdio.isatty("stdin") then
  payload = airsstack.hook.payload()
end
if next(payload) == nil then
  payload = airsstack.json.decode(airsstack.fs.read(airsstack.path.join(here, "payload.json")))
end

local event = payload.hook_event_name
local input = payload.tool_input or {}
local file = input.file_path or ""

-- Every line this script prints goes through `airsstack.stdio`, never Lua's own `print`. Not for
-- ordering -- both flush per line, so they interleave correctly -- but because `print` appends a
-- newline and cannot be told not to. On this stream the bytes are the interface, and a writer that
-- adds one of its own is the wrong tool whatever it is being asked to write.
airsstack.stdio.write(("-- payload: event=%s tool=%s file=%s\n"):format(event, payload.tool_name, file))

if airsstack.path.ext(file) == "lua" then
  airsstack.stdio.write("-- hook.context: the envelope, nesting included\n")

  -- The nesting is written once, in Rust (src/modules/hook.rs:108), so that no script has to
  -- remember which key goes inside which. What it will not build is a `permissionDecision` field:
  -- that field is honoured on a path that can swallow the tool call the hook fired on outright,
  -- leaving the caller with no `tool_result` at all. `additionalContext` alone injects the note
  -- into the model's turn and lets the tool run.
  airsstack.hook.context(event, "This file is an airsl hook: it runs under a policy, and its authority comes from the invocation.")

  -- `context` and `emit` add no trailing newline (src/modules/stdio.rs:98), because a parser
  -- cares what the bytes are. This example prints two documents, so it separates them itself; a
  -- real hook writes one and adds nothing to it.
  airsstack.stdio.write("\n")
end

-- `emit` imposes no shape at all -- it is the escape hatch for whatever `context` does not model.
airsstack.stdio.write("-- hook.emit: any document, and exactly the bytes handed to it\n")
airsstack.hook.emit({ reviewed = file, suppressOutput = false })
airsstack.stdio.write("\n")

-- Nothing here exits, and nothing here can. Only the process's exit status blocks the tool call a
-- PreToolUse hook fired on; a message on stdout or stderr never does. That status belongs to the
-- host and its FailurePolicy, which is why a hook that disagrees with a tool call still returns
-- normally and says so in the envelope.
