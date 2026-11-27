@echo off
rem Builds the two Windows Lua interpreters the native module is checked against:
rem Lua 5.4.8 into C:\lua54 and LuaJIT into C:\luajit, each with bin, lib and include.
rem
rem It is one script because every job that needs these interpreters must test the same
rem ones; two copies of a build recipe drift, and a module built against one interpreter
rem would then be checked against another.
rem
rem It must run in an MSVC developer prompt (`cl` and `link` on PATH), which on a runner
rem means after ilammy/msvc-dev-cmd. Nothing here installs luarocks.
rem
rem Every download is pinned, and every step stops the script on failure: a half-built
rem interpreter that "mostly works" would turn a toolchain fault into a confusing
rem native-load failure later. LuaJIT is pinned to a commit, not the moving v2.1 branch,
rem so the interpreter does not change under a green build. The checkout is a full clone
rem rather than a shallow one, because the pinned commit stops being the branch tip as
rem soon as upstream moves on. `git checkout` of a full commit id either lands on that
rem commit or fails, so no second check of HEAD follows it.

setlocal EnableExtensions DisableDelayedExpansion

set "LUA_TARBALL=lua-5.4.8.tar.gz"
set "LUA_SHA256=4f18ddae154e793e46eeab727c59ef1c0c0c2b744e7b94219710d76f530629ae"
set "LUAJIT_COMMIT=96f7b297326e9fd454b7186c6858b2773f94a0dc"

set "STEP=find the MSVC compiler on PATH, run this inside an MSVC developer prompt"
where cl >NUL 2>&1 || goto :fail

if not defined RUNNER_TEMP set "RUNNER_TEMP=%TEMP%"
set "WORK=%RUNNER_TEMP%\msvc-lua"
set "STEP=prepare the work directory %WORK%"
if exist "%WORK%" rmdir /s /q "%WORK%"
mkdir "%WORK%" || goto :fail
pushd "%WORK%" || goto :fail

rem ---- Lua 5.4.8 -----------------------------------------------------------------------

set "STEP=download %LUA_TARBALL%"
curl -sSfL --retry 3 -o "%LUA_TARBALL%" "https://www.lua.org/ftp/%LUA_TARBALL%" || goto :fail

rem certutil prints a header line, the hash, then a status line; the hash is the first
rem line after the header. It may be space-separated on older Windows.
set "STEP=read the SHA-256 of %LUA_TARBALL%"
set "GOT="
for /f "skip=1 tokens=*" %%h in ('certutil -hashfile "%LUA_TARBALL%" SHA256') do if not defined GOT set "GOT=%%h"
if not defined GOT goto :fail
set "GOT=%GOT: =%"
set "STEP=verify the SHA-256 of %LUA_TARBALL%: expected %LUA_SHA256%, got %GOT%"
if /i not "%GOT%"=="%LUA_SHA256%" goto :fail

set "STEP=unpack %LUA_TARBALL%"
tar -xzf "%LUA_TARBALL%" || goto :fail

rem LUA_BUILD_AS_DLL exports the API from lua54.dll, which is what a native module links
rem against. lua.c and luac.c hold the two programs' main(); they are renamed out of the
rem *.obj glob so the DLL does not get a second entry point, and lua.exe links the DLL.
pushd lua-5.4.8\src || goto :fail
set "STEP=compile Lua 5.4.8 with cl"
cl /nologo /MD /O2 /W3 /c /DLUA_BUILD_AS_DLL *.c >cl.log 2>&1 || (type cl.log & goto :fail)
set "STEP=rename lua.obj and luac.obj out of the *.obj glob"
ren lua.obj lua.o || goto :fail
ren luac.obj luac.o || goto :fail
set "STEP=link lua54.dll"
link /nologo /DLL /IMPLIB:lua54.lib /OUT:lua54.dll *.obj || goto :fail
set "STEP=link lua.exe"
link /nologo /OUT:lua.exe lua.o lua54.lib || goto :fail

set "STEP=install Lua 5.4.8 into C:\lua54"
if not exist C:\lua54\bin mkdir C:\lua54\bin || goto :fail
if not exist C:\lua54\lib mkdir C:\lua54\lib || goto :fail
if not exist C:\lua54\include mkdir C:\lua54\include || goto :fail
copy /y lua.exe C:\lua54\bin\ >NUL || goto :fail
copy /y lua54.dll C:\lua54\bin\ >NUL || goto :fail
copy /y lua54.lib C:\lua54\lib\ >NUL || goto :fail
for %%h in (lua.h luaconf.h lualib.h lauxlib.h lua.hpp) do copy /y %%h C:\lua54\include\ >NUL || goto :fail
popd

rem ---- LuaJIT --------------------------------------------------------------------------

set "STEP=clone LuaJIT"
git clone -q --branch v2.1 https://github.com/LuaJIT/LuaJIT.git luajit-src || goto :fail
set "STEP=check out LuaJIT commit %LUAJIT_COMMIT%"
git -C luajit-src checkout -q %LUAJIT_COMMIT% || goto :fail

rem msvcbuild.bat builds the DLL flavour by default: luajit.exe plus lua51.dll, which is
rem the name a LuaJIT-targeting module links against.
pushd luajit-src\src || goto :fail
set "STEP=build LuaJIT with msvcbuild.bat"
call msvcbuild.bat >msvcbuild.log 2>&1 || (type msvcbuild.log & goto :fail)
rem The exit code of a third-party batch file is not trusted on its own, so the two
rem products are checked by name as well.
if not exist luajit.exe (type msvcbuild.log & goto :fail)
if not exist lua51.dll (type msvcbuild.log & goto :fail)

set "STEP=install LuaJIT into C:\luajit"
if not exist C:\luajit\bin mkdir C:\luajit\bin || goto :fail
if not exist C:\luajit\lib mkdir C:\luajit\lib || goto :fail
if not exist C:\luajit\include mkdir C:\luajit\include || goto :fail
copy /y luajit.exe C:\luajit\bin\ >NUL || goto :fail
copy /y lua51.dll C:\luajit\bin\ >NUL || goto :fail
copy /y lua51.lib C:\luajit\lib\ >NUL || goto :fail
for %%h in (lua.h luaconf.h lualib.h lauxlib.h luajit.h lua.hpp) do copy /y %%h C:\luajit\include\ >NUL || goto :fail
popd

rem ---- Prove they run ------------------------------------------------------------------

rem The version lines are the record of what the gate ran against.
popd
set "STEP=run C:\lua54\bin\lua.exe -v"
C:\lua54\bin\lua.exe -v || goto :fail
set "STEP=run C:\luajit\bin\luajit.exe -v"
C:\luajit\bin\luajit.exe -v || goto :fail
exit /b 0

:fail
echo msvc-lua: failed to %STEP% 1>&2
exit /b 1
