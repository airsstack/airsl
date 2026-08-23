-- Registers "count", the one event this host declares. The manifest asks for `regex = true`
-- because plain `string.gmatch` patterns cannot express "a run of word characters" without
-- hand-rolling character classes; `\w+` says it directly and is the reason the capability line
-- exists.
airsstack.ext.on("count", function(payload)
    local words = airsstack.regex.find_all([[\w+]], payload.text)

    local longest = ""
    for _, word in ipairs(words) do
        if #word > #longest then
            longest = word
        end
    end

    return { words = #words, longest = longest }
end)
