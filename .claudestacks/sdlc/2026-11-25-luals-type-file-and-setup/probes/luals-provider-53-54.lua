            log.info(inspect(clientConfig))
        end

        local rc = cfgLoader.loadRCConfig(folder.uri, '.luarc.json')
                or cfgLoader.loadRCConfig(folder.uri, '.luarc.jsonc')
        if rc then
            log.info('Load config from .luarc.json/.luarc.jsonc', folder.uri)
            log.info(inspect(rc))
        end
