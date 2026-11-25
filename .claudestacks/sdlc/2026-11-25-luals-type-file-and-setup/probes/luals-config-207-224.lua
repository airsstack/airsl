-- lines 207-224 of LuaLS 3.19.2 libexec/script/config/config.lua (Mason install)
    local function expand(t, left)
        for key, val in pairs(t) do
            local value = val
            local fullKey = key
            if left then
                fullKey = left .. '.' .. key
            end
            if m.nullSymbols[value] then
                value = m.NULL
            end
            if     template[fullKey] then
                m.setByScope(scp, fullKey, value)
            elseif template['Lua.' .. fullKey] then
                m.setByScope(scp, 'Lua.' .. fullKey, value)
            elseif type(value) == 'table' then
                expand(value, fullKey)
            end
        end
