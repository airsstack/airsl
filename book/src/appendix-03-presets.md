# C. Presets at a Glance

| | `trusted` | `confined` *(default)* | `pure` |
|---|---|---|---|
| Meant for | your own scripts | everyday and third-party scripts | pure calculations |
| Rust | `Policy::trusted()` | `Policy::confined()` | `Policy::pure()` |
| `doctor` language | `full` | `restricted` | `minimal` |
| Lua libraries | everything except `debug` | `string`, `table`, `math`, `utf8`, `coroutine`, safe `os` | `string`, `table`, `math`, `utf8` |
| `io`, `package` | yes | no | no |
| `load`, `loadstring`, `dofile`, `loadfile` | yes | no | no |
| `os.execute`, `exit`, `getenv`, `remove`, `rename`, `tmpname`, `setlocale` | yes | no | no |
| `os.time`, `os.clock`, `os.date` | yes | yes | no |
| `require` | Lua's own | the script's folder only | none |
| `airsstack` modules | all, unrestricted | all, only what is granted | all, only what is granted |
| Memory limit | none | 64 MiB | 16 MiB |
| Instruction limit | none | 100 million | 10 million |

`utf8` is on every preset. Without it a script could only count bytes, not characters, and could
not shorten text without the risk of cutting a character in half.
