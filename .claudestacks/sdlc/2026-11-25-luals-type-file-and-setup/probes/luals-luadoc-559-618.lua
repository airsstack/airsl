-- lines 559-618 of LuaLS 3.19.2 libexec/script/parser/luadoc.lua (Mason install)
        if checkToken('symbol', ':', 1) then
            nextToken()
            arg.extends = parseType(arg)
        end
        arg.finish = getFinish()
        typeUnit.args[#typeUnit.args+1] = arg
        if checkToken('symbol', ',', 1) then
            nextToken()
        else
            nextSymbolOrError(')')
            break
        end
    end
    slideToNextLine()
    if checkToken('symbol', ':', 1) then
        nextToken()
        slideToNextLine()
        local needCloseParen
        if checkToken('symbol', '(', 1) then
            nextToken()
            needCloseParen = true
        end
        while true do
            slideToNextLine()
            local name
            try(function ()
                local returnName = parseName('doc.return.name', typeUnit)
                                or parseDots('doc.return.name', typeUnit)
                if not returnName then
                    return false
                end
                if checkToken('symbol', ':', 1) then
                    nextToken()
                    name = returnName
                    return true
                end
                if returnName[1] == '...' then
                    name = returnName
                    return false
                end
                return false
            end)
            local rtn = parseType(typeUnit)
            if not rtn then
                break
            end
            rtn.name = name
            if checkToken('symbol', '?', 1) then
                nextToken()
                rtn.optional = true
            end
            typeUnit.returns[#typeUnit.returns+1] = rtn
            if checkToken('symbol', ',', 1) then
                nextToken()
            else
                break
            end
        end
        if needCloseParen then
            nextSymbolOrError ')'
