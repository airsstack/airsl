use airsl::{Engine, Policy};
fn main() {
    let src = r#"
      local out = {}
      local root = airsstack
      local mods = {}
      for k in pairs(root) do mods[#mods+1] = k end
      table.sort(mods)
      for _, m in ipairs(mods) do
        local keys = {}
        for k, v in pairs(root[m]) do keys[#keys+1] = k .. ":" .. type(v) end
        table.sort(keys)
        out[#out+1] = m .. " (" .. #keys .. ") " .. table.concat(keys, " ")
      end
      local mt = getmetatable(root)
      out[#out+1] = "root metatable: " .. tostring(mt)
      return table.concat(out, "\n")
    "#;
    for (name, p) in [("trusted", Policy::trusted()), ("confined", Policy::confined()), ("pure", Policy::pure())] {
        let e = Engine::builder().policy(p).build().unwrap();
        let s: String = e.eval_to(&airsl::Script::from_source(src, "probe").unwrap()).unwrap();
        println!("== {name}\n{s}");
    }
}
