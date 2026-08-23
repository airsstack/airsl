-- The twin of `toolkit.lua`, run by `airsl test` rather than by the Rust host.
--
-- A test file is named `*_test.lua` or `test_*.lua`, returns a table whose named function values
-- are the tests, and reports by control flow: a test passes by returning and fails by raising. So
-- `assert` is the entire assertion surface, and there is no framework to learn.
--
-- Every module used below needs no grant, which is what lets `airsl test` run this directory with
-- no `--allow-*` flag at all. A test that needed one would be testing the policy as much as the
-- code, and would fail for whoever ran the obvious command.

local regex = airsstack.regex
local path = airsstack.path
local hash = airsstack.hash
local time = airsstack.time
local glob = airsstack.glob

local LINE = "2023-11-14T22:13:21Z WARN  fs denied: /etc/shadow is outside the granted read roots"
local ENTRY = [[^(?<stamp>\S+) (?<level>INFO|WARN|ERROR)\s+(?<module>\w+) (?<message>.*)$]]

return {
  a_compiled_pattern_carries_the_same_operations_as_the_one_shot_forms = function()
    local entry = regex.compile(ENTRY)
    assert(entry.is_match(LINE))
    assert(entry.find(LINE) == LINE)
    assert(#entry.find_all(LINE) == 1)
    assert(entry.captures(LINE).level == "WARN")

    local spaces = regex.compile([[\s+]])
    assert(#spaces.split("a  b c") == 3)
    assert(spaces.replace_all("a  b", "-") == "a-b")
  end,

  a_compiled_pattern_is_reusable_across_calls = function()
    -- The reason `compile` exists: the handle outlives the call that made it, so a scan pays the
    -- compilation once rather than once per line.
    local digits = regex.compile([[\d+]])
    local total = 0
    for _, text in ipairs({ "a1", "b22", "c333" }) do
      total = total + #digits.find(text)
    end
    assert(total == 6, total)
  end,

  captures_arrive_under_their_number_and_their_name = function()
    local caught = regex.captures(ENTRY, LINE)
    assert(caught[0] == LINE)
    assert(caught[2] == caught.level)
    assert(caught[3] == caught.module)
  end,

  alternation_and_word_boundaries_work_where_a_lua_pattern_has_neither = function()
    assert(regex.is_match([[^(cat|dog)$]], "dog"))
    assert(regex.is_match([[\bcat\b]], "the cat sat"))
    assert(not regex.is_match([[\bcat\b]], "concatenate"))
  end,

  a_replacement_can_refer_to_a_capture = function()
    assert(regex.replace_all([[(denied: )/\S+]], LINE, "${1}<path>") ==
      "2023-11-14T22:13:21Z WARN  fs denied: <path> is outside the granted read roots")
  end,

  an_invalid_pattern_raises_rather_than_returning_a_sentinel = function()
    assert(not pcall(regex.compile, "(unclosed"))
  end,

  path_splits_a_name_into_a_stem_and_an_extension = function()
    local target = path.join("crates", "airsl", "examples", "toolkit.lua")
    assert(path.dirname(target) == "crates/airsl/examples")
    assert(path.basename(target) == "toolkit.lua")
    assert(path.stem(target) == "toolkit")
    assert(path.ext(target) == "lua")
  end,

  a_missing_extension_is_an_empty_string_and_not_nil = function()
    -- Absence and failure are never the same value, so the result concatenates without a check.
    assert(path.ext("README") == "")
  end,

  normalize_resolves_dots_without_consulting_the_filesystem = function()
    -- None of these paths exists. That is the point: `path` needs no grant because it never asks.
    assert(path.normalize("a/b/../c/./d") == "a/c/d")
    assert(path.normalize("../a") == "../a")
  end,

  relative_to_refuses_a_path_that_is_not_under_its_base = function()
    assert(path.relative_to("a/b/c", "a") == "b/c")
    assert(not pcall(path.relative_to, "a/b", "c"))
  end,

  is_absolute_reads_the_leading_separator_and_nothing_else = function()
    assert(path.is_absolute("/etc/hosts"))
    assert(not path.is_absolute("etc/hosts"))
  end,

  hashing_reproduces_the_digests_the_command_line_tools_print = function()
    assert(hash.sha256("abc") == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    assert(hash.sha1("abc") == "a9993e364706816aba3e25717850c26c9cd0d89d")
    assert(hash.hex("abc") == "616263")
  end,

  the_same_input_hashes_to_the_same_digest = function()
    -- Content addressing is only worth anything if this holds, and it is the property a cache key
    -- derived from a hash silently depends on.
    assert(hash.sha256(LINE) == hash.sha256(LINE))
  end,

  time_formats_in_utc_so_the_output_does_not_depend_on_the_machine = function()
    assert(time.format(0) == "1970-01-01T00:00:00Z")
    assert(time.format(1700000000, "%Y-%m-%dT%H:%M:%S") == "2023-11-14T22:13:20")
  end,

  parse_reads_back_what_format_wrote = function()
    assert(time.parse(time.format(1700000000)) == 1700000000)
    assert(time.parse("2023-11-14T22:13:20Z") == 1700000000)
  end,

  parsing_something_that_is_not_a_date_raises = function()
    assert(not pcall(time.parse, "not a date"))
  end,

  monotonic_does_not_go_backwards = function()
    -- The reading itself differs on every run, so the property is what a test can hold onto.
    local before = time.monotonic()
    local sum = 0
    for i = 1, 10000 do
      sum = sum + i
    end
    assert(sum == 50005000)
    assert(time.monotonic() >= before)
  end,

  a_glob_star_stops_at_a_directory_boundary = function()
    -- The looser reading would let a rule declaring `*.lua` apply to every Lua file in a tree
    -- rather than the ones its author named.
    assert(glob.match("*.lua", "toolkit.lua"))
    assert(not glob.match("*.lua", "examples/toolkit.lua"))
  end,

  a_glob_double_star_matches_zero_segments_as_well_as_many = function()
    assert(glob.match("**/toolkit.lua", "toolkit.lua"))
    assert(glob.match("**/toolkit.lua", "crates/airsl/examples/toolkit.lua"))
  end,
}
