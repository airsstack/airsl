// Reviewer probe: does table.sort on the Minimal surface give the same order for the same input?
// Builds an input that unbalances Lua 5.4's first quicksort partitions (McIlroy's adversary),
// then sorts records with tied keys taken from it, many times, and counts distinct results.
use mlua::{Lua, LuaOptions, StdLib};

fn main() {
    let lua = Lua::new_with(StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8, LuaOptions::default()).unwrap();
    let out: String = lua
        .load(r#"
            local n = 3000
            local val, gas, nsolid, candidate = {}, n + 1, 0, nil
            local ptr = {}
            for i = 1, n do ptr[i] = i; val[i] = gas end
            local function freeze(x) val[x] = nsolid; nsolid = nsolid + 1 end
            table.sort(ptr, function(x, y)
              if val[x] == gas and val[y] == gas then
                if x == candidate then freeze(x) else freeze(y) end
              end
              if val[x] == gas then candidate = x elseif val[y] == gas then candidate = y end
              return val[x] < val[y]
            end)
            local lines = {}
            for _, div in ipairs({1, 2, 4}) do
              local seen, distinct = {}, 0
              for round = 1, 100 do
                local t = {}
                for i = 1, n do t[i] = { k = val[i] // div, id = i } end
                table.sort(t, function(a, b) return a.k < b.k end)
                local ids = {}
                for i = 1, n do ids[i] = t[i].id end
                local s = table.concat(ids, ",")
                if not seen[s] then seen[s] = true; distinct = distinct + 1 end
                local burn = 0 for j = 1, 200000 do burn = burn + j end
              end
              lines[#lines + 1] = "key//" .. div .. ": distinct orders over 100 sorts of identical input: " .. distinct
            end
            return table.concat(lines, "\n")
        "#)
        .eval()
        .unwrap();
    println!("{out}");
}
