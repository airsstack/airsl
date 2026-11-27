//! What `airsl lua setup --native` installs, and what each way it can fail says.
//!
//! Its own file because the decision is pure: which package matches the Lua luarocks targets, the
//! argument list that asks luarocks, and the words of every refusal. It spawns nothing, so each
//! refusal is tested with a scripted [`Luarocks`] and on every OS through [`Flavour`].
//!
//! Responsibilities: [`Package`], [`NativeRequest`], [`Flavour`], reading luarocks' answers to
//! `config lua_version` and, for Lua 5.1, `config cache` ([`setup_native`]), the `install --local`
//! call ([`install_native`]), the `luarocks path` hint ([`hint`]) and [`NativeError`], whose text
//! is what the user reads after `airsl: `.
//!
//! Non-responsibilities: spawning a process (`luarocks`), and printing and the order of the
//! stages (`command`).
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::io;

use super::luarocks::{Luarocks, LuarocksError, PROGRAM, WINDOWS_PROGRAM};
use crate::cli::{LUA54_FLAG, LuaAbi};

/// The luarocks subcommand that prints one configuration value. `config <key>` is how luarocks
/// reports the Lua it targets, and it exits 0 whether or not that Lua is installed.
const CONFIG: &str = "config";

/// The configuration key that holds the Lua version luarocks targets, such as `5.4`.
const LUA_VERSION_KEY: &str = "lua_version";

/// The configuration key that holds what luarocks has found out about the interpreter it targets.
/// Printing it is how luarocks reports whether it found a `LuaJIT` for Lua 5.1: the table holds
/// [`LUAJIT_VERSION_KEY`] only when it did, and holds [`LUAJIT_CHECKED_KEY`] once it looked.
const CACHE_KEY: &str = "cache";

/// The entry of the `cache` table that is the `LuaJIT` version, present only for `LuaJIT`.
const LUAJIT_VERSION_KEY: &str = "luajit_version";

/// The entry of the `cache` table that says luarocks has looked for a `LuaJIT`, present whatever
/// it found. luarocks sets it before it checks that an interpreter exists at all, so it is also
/// present when there is no Lua 5.1 interpreter to ask.
const LUAJIT_CHECKED_KEY: &str = "luajit_version_checked";

/// The luarocks flag that chooses the Lua to target for one invocation, in place of the
/// configured one. Every call puts it before the subcommand, where luarocks reads its global
/// flags.
const LUA_VERSION_FLAG: &str = "--lua-version";

/// The `lua_version` luarocks reports for Lua 5.4, and the value `--lua-version` takes for it.
const LUA54_VERSION: &str = "5.4";

/// The `lua_version` luarocks reports for Lua 5.1, which is the ABI `LuaJIT` implements, and the
/// value `--lua-version` takes for it.
const LUA51_VERSION: &str = "5.1";

/// The luarocks subcommand that installs a rock by name and version.
const INSTALL: &str = "install";

/// The luarocks flag that installs into the user's own tree, so no administrator rights are
/// needed.
const LOCAL: &str = "--local";

/// The luarocks subcommand that prints the shell commands that put the installed rocks on the
/// search paths.
const PATH: &str = "path";

/// The batch file the Windows hint has `luarocks path` write, for `cmd` to run.
const LRPATH_BAT: &str = "lrpath.bat";

/// The book page that says what works on musl Linux and where no package exists.
const BOOK_PAGE: &str = "https://airsstack.github.io/airsl/ch01-05-stock-lua.html";

/// The book section that explains how to point luarocks at Lua 5.4 or `LuaJIT`.
const BOOK_POINTING: &str = "https://airsstack.github.io/airsl/ch01-05-stock-lua.html#pointing-luarocks-at-lua-54-or-luajit";

/// The width of `airsl: `, which continuation lines of a message are indented by so they line up
/// under its first word.
const CONTINUATION: &str = "       ";

/// Why a `.bat` wrapper does not help on Windows: airsl starts `luarocks.exe` by name, and a
/// wrapper is not found that way.
const WINDOWS_WRAPPER_NOTE: &str = "airsl runs `luarocks.exe` itself, not a `luarocks.bat` wrapper";

/// The native package to install, one per Lua ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Package {
    /// The package for Lua 5.4.
    Airsstack,
    /// The package for `LuaJIT`, which implements the Lua 5.1 ABI.
    AirsstackLuajit,
}

impl Package {
    /// The name luarocks knows the package by.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Airsstack => "airsstack",
            Self::AirsstackLuajit => "airsstack-luajit",
        }
    }
}

impl From<LuaAbi> for Package {
    fn from(abi: LuaAbi) -> Self {
        match abi {
            LuaAbi::Lua54 => Self::Airsstack,
            LuaAbi::LuaJit => Self::AirsstackLuajit,
        }
    }
}

/// What the user asked `--native` to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeRequest {
    /// The Lua to install for, or `None` for the one luarocks targets.
    pub lua: Option<LuaAbi>,
}

/// Which operating system's wording a message uses. The caller passes it in, so both texts are
/// tested on every OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flavour {
    /// Unix shells and a `luarocks` program.
    Unix,
    /// Windows, where the program is `luarocks.exe`.
    Windows,
}

impl Flavour {
    /// The wording for the operating system this binary was built for.
    pub(crate) const fn host() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }

    /// The file name the operating system starts for luarocks.
    const fn program(self) -> &'static str {
        match self {
            Self::Unix => PROGRAM,
            Self::Windows => WINDOWS_PROGRAM,
        }
    }
}

/// Why the native module could not be installed. `Display` is the message without the
/// `airsl: ` prefix, which the caller adds.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NativeError {
    /// No luarocks program was found.
    #[error("{}", missing_text(*flavour, *lua))]
    Missing {
        /// Whose wording to use.
        flavour: Flavour,
        /// The `--lua` the user gave, which narrows the install lines to one.
        lua: Option<LuaAbi>,
    },
    /// luarocks exists but could not be started.
    #[error("run failed on `{}`: {source}", flavour.program())]
    Spawn {
        /// Whose wording to use.
        flavour: Flavour,
        /// The operating system's reason.
        #[source]
        source: io::Error,
    },
    /// A signal ended `config lua_version` before it answered.
    #[error("`{command}` was stopped by a signal")]
    Signal {
        /// The command that ran.
        command: String,
    },
    /// `config lua_version` exited with a non-zero status.
    #[error("`{command}` failed with exit status {code}")]
    Failed {
        /// The command that ran.
        command: String,
        /// Its exit status.
        code: i32,
    },
    /// `config lua_version` printed nothing.
    #[error("`{command}` printed no Lua version")]
    NoVersion {
        /// The command that ran.
        command: String,
    },
    /// `config lua_version` printed something that is not `digits.digits`.
    #[error("`{command}` printed `{shown}`, which is not a Lua version")]
    NotAVersion {
        /// The command that ran.
        command: String,
        /// What it printed, escaped with `escape_debug` so control and invisible characters are
        /// visible. A backtick is not touched by that, so one in the answer shows as it is.
        shown: String,
    },
    /// luarocks found no `LuaJIT` for Lua 5.1: the Lua 5.1 it targets is not `LuaJIT`, or there is
    /// no Lua 5.1 interpreter. Either way the 5.1 package has nothing to load into.
    #[error(
        "`luarocks` found no LuaJIT for Lua 5.1: its Lua 5.1 is not LuaJIT, or it has none, and \
         `airsstack-luajit` needs LuaJIT\n\
         {CONTINUATION}install Lua 5.4 and run again with `--lua {LUA54_FLAG}` to install it for \
         Lua 5.4\n\
         {CONTINUATION}or run `{PROGRAM} {LUA_VERSION_FLAG} {LUA51_VERSION} config --scope user \
         lua_dir <LuaJIT prefix>` to point `luarocks` at LuaJIT\n\
         {CONTINUATION}{BOOK_POINTING} shows how to point `luarocks` at Lua 5.4 or LuaJIT"
    )]
    NoLuajit,
    /// luarocks targets a Lua that no native package supports.
    #[error(
        "`luarocks` installs for Lua {version}, and `airsstack` is built for Lua 5.4 and LuaJIT\n\
         {CONTINUATION}run again with `--lua {LUA54_FLAG}` to install it for Lua 5.4\n\
         {CONTINUATION}{BOOK_POINTING} shows how to point `luarocks` at Lua 5.4 or LuaJIT"
    )]
    Unsupported {
        /// The version luarocks reported.
        version: String,
    },
    /// `--lua` named one Lua and luarocks answered for another.
    #[error("`{command}` printed `{version}`, not the {wanted} that `--lua {flag}` asks for")]
    Disagrees {
        /// The command that ran, with the `--lua-version` that was passed.
        command: String,
        /// The version luarocks reported.
        version: String,
        /// The version `--lua` asks luarocks for.
        wanted: &'static str,
        /// The `--lua` value the user gave.
        flag: &'static str,
    },
    /// The install command exited with a non-zero status. airsl's own exit status stays 1.
    #[error("`{command}` failed with exit status {code}")]
    InstallFailed {
        /// The command that ran.
        command: String,
        /// luarocks' exit status.
        code: i32,
    },
    /// A signal ended the install command.
    #[error("`{command}` was stopped by a signal")]
    InstallSignal {
        /// The command that ran.
        command: String,
    },
}

/// The text for a missing luarocks: the command that would install the package. On Windows a line
/// follows the first that says why a `.bat` wrapper does not do.
fn missing_text(flavour: Flavour, lua: Option<LuaAbi>) -> String {
    let first = format!("`{}` was not found on PATH", flavour.program());
    let mut rest = Vec::new();
    if flavour == Flavour::Windows {
        rest.push(WINDOWS_WRAPPER_NOTE.to_owned());
    }
    match lua {
        None => {
            rest.push("install LuaRocks, then run the line for your Lua:".to_owned());
            rest.push(install_command(Some(LuaAbi::Lua54), Package::Airsstack));
            rest.push(install_command(
                Some(LuaAbi::LuaJit),
                Package::AirsstackLuajit,
            ));
        }
        Some(abi) => rest.push(format!(
            "install LuaRocks, then run: {}",
            install_command(Some(abi), Package::from(abi))
        )),
    }
    let indented = rest.iter().map(|line| format!("\n{CONTINUATION}{line}"));
    std::iter::once(first).chain(indented).collect()
}

/// Maps a failure to start luarocks to the refusal that names it.
fn launch_error(error: LuarocksError, request: NativeRequest, flavour: Flavour) -> NativeError {
    match error {
        LuarocksError::Missing => NativeError::Missing {
            flavour,
            lua: request.lua,
        },
        LuarocksError::Spawn { source } => NativeError::Spawn { flavour, source },
    }
}

/// The `--lua-version` value, and the `config lua_version` answer, for a Lua.
const fn lua_version(abi: LuaAbi) -> &'static str {
    match abi {
        LuaAbi::Lua54 => LUA54_VERSION,
        LuaAbi::LuaJit => LUA51_VERSION,
    }
}

/// The arguments of one luarocks call: `--lua-version` first when a Lua was chosen, then `tail`.
/// The call that is spawned and the command a message names are both built from this, so they
/// cannot differ.
fn argv(lua: Option<LuaAbi>, tail: &[&'static str]) -> Vec<&'static str> {
    let mut args = Vec::with_capacity(tail.len() + 2);
    if let Some(abi) = lua {
        args.extend([LUA_VERSION_FLAG, lua_version(abi)]);
    }
    args.extend_from_slice(tail);
    args
}

/// The command line a user would type to run `args`.
fn render(args: &[&str]) -> String {
    let mut line = String::from(PROGRAM);
    for arg in args {
        line.push(' ');
        line.push_str(arg);
    }
    line
}

/// The command a user types to install `package`, for the Lua `lua` names.
fn install_command(lua: Option<LuaAbi>, package: Package) -> String {
    render(&install_args(lua, package))
}

/// The arguments of the install call: `--lua-version` when a Lua was chosen, then `install
/// --local <package> <airsl version>`.
fn install_args(lua: Option<LuaAbi>, package: Package) -> Vec<&'static str> {
    argv(lua, &[INSTALL, LOCAL, package.as_str(), airsl::VERSION])
}

/// Asks luarocks which Lua it targets and picks the package for it.
///
/// Runs `config lua_version`, with `--lua-version` first when the request names a Lua. The answer
/// is judged in this order: a spawn error, a signal, a non-zero status, and only then the text
/// luarocks printed, so a failed command is never read for a version.
///
/// A Lua 5.1 takes the `LuaJIT` package only when luarocks found a `LuaJIT` for it, which it
/// reports by printing `config cache`. The refusal is made here, before the caller writes any
/// file, because the `LuaJIT` package's own dependency on `luajit` fails the install later. A
/// `config cache` answer that cannot be read as that table leaves the choice as it was, so a
/// luarocks that reports less than this one does is no worse off than before.
///
/// # Errors
///
/// [`NativeError::Missing`] or [`NativeError::Spawn`] when luarocks cannot be started,
/// [`NativeError::Signal`] or [`NativeError::Failed`] when it ends badly,
/// [`NativeError::NoVersion`] or [`NativeError::NotAVersion`] when its answer is not a version,
/// [`NativeError::Unsupported`] when no package matches the version,
/// [`NativeError::Disagrees`] when `--lua` named another Lua than the one it answered, and
/// [`NativeError::NoLuajit`] when luarocks found no `LuaJIT` for the Lua 5.1 it targets.
pub(crate) fn setup_native<L: Luarocks>(
    luarocks: &L,
    request: NativeRequest,
    flavour: Flavour,
) -> Result<Package, NativeError> {
    let args = argv(request.lua, &[CONFIG, LUA_VERSION_KEY]);
    let command = render(&args);
    let captured = luarocks
        .query(&args)
        .map_err(|error| launch_error(error, request, flavour))?;
    let package = match captured.code {
        None => return Err(NativeError::Signal { command }),
        Some(code) if code != 0 => return Err(NativeError::Failed { command, code }),
        Some(_) => choose_package(&captured.stdout, request, command)?,
    };
    if package == Package::AirsstackLuajit {
        require_luajit(luarocks, request, flavour)?;
    }
    Ok(package)
}

/// Refuses a Lua 5.1 for which luarocks found no `LuaJIT`.
///
/// Runs `config cache`. Only a table that holds [`LUAJIT_CHECKED_KEY`] and not
/// [`LUAJIT_VERSION_KEY`] is a refusal: luarocks looked, and either its Lua 5.1 is not `LuaJIT` or
/// there is no Lua 5.1 interpreter. Anything else says nothing about the interpreter, so the
/// install goes on: a command that ran and failed, or printed a table without the check. A command
/// that cannot start is the exception, and is refused as `config lua_version` is.
fn require_luajit<L: Luarocks>(
    luarocks: &L,
    request: NativeRequest,
    flavour: Flavour,
) -> Result<(), NativeError> {
    let args = argv(request.lua, &[CONFIG, CACHE_KEY]);
    let captured = luarocks
        .query(&args)
        .map_err(|error| launch_error(error, request, flavour))?;
    if captured.code != Some(0) {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&captured.stdout);
    let has = |wanted: &str| text.lines().any(|line| entry_key(line) == wanted);
    if has(LUAJIT_CHECKED_KEY) && !has(LUAJIT_VERSION_KEY) {
        return Err(NativeError::NoLuajit);
    }
    Ok(())
}

/// The key a line of a printed Lua table sets: the identifier it starts with, after any
/// indentation, and nothing when the line does not start with one.
fn entry_key(line: &str) -> &str {
    let line = line.trim_start();
    let end = line
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(line.len());
    &line[..end]
}

/// Installs `package` for the user with luarocks, at airsl's own version.
///
/// Runs `install --local <package> <version>`, with `--lua-version` first when the request names
/// a Lua. luarocks writes to the terminal itself; airsl adds a line only when it fails.
///
/// # Errors
///
/// [`NativeError::Missing`] or [`NativeError::Spawn`] when luarocks cannot be started,
/// [`NativeError::InstallSignal`] when a signal ends it, and [`NativeError::InstallFailed`] for
/// any non-zero exit status, which it carries for the message only: airsl exits 1 either way.
pub(crate) fn install_native<L: Luarocks>(
    luarocks: &L,
    request: NativeRequest,
    package: Package,
    flavour: Flavour,
) -> Result<(), NativeError> {
    let args = install_args(request.lua, package);
    let command = render(&args);
    let outcome = luarocks
        .run(&args)
        .map_err(|error| launch_error(error, request, flavour))?;
    match outcome.code {
        Some(0) => Ok(()),
        Some(code) => Err(NativeError::InstallFailed { command, code }),
        None => Err(NativeError::InstallSignal { command }),
    }
}

/// The lines that tell the user how to make `require("airsstack")` find the installed package,
/// without a trailing newline.
///
/// Installing with `--local` puts the package outside the default search paths, and a program
/// cannot change its parent shell's environment, so the user runs `luarocks path` themselves:
/// one line on Unix, the `cmd` and `PowerShell` forms on Windows.
pub(crate) fn hint(request: NativeRequest, flavour: Flavour) -> String {
    let path = render(&argv(request.lua, &[PATH]));
    match flavour {
        Flavour::Unix => {
            format!("require(\"airsstack\") works once you run: eval \"$({path})\"")
        }
        Flavour::Windows => format!(
            "require(\"airsstack\") works once luarocks' paths are set. In cmd:\n  \
             {path} > {LRPATH_BAT}\n  \
             call {LRPATH_BAT}\n\
             In PowerShell:\n  \
             $env:LUA_PATH = {path} --lr-path\n  \
             $env:LUA_CPATH = {path} --lr-cpath"
        ),
    }
}

/// Picks the package for what `config lua_version` printed.
fn choose_package(
    stdout: &[u8],
    request: NativeRequest,
    command: String,
) -> Result<Package, NativeError> {
    let trimmed = stdout.trim_ascii();
    if trimmed.is_empty() {
        return Err(NativeError::NoVersion { command });
    }
    let Some(version) = std::str::from_utf8(trimmed)
        .ok()
        .filter(|text| is_version(text))
    else {
        let shown = String::from_utf8_lossy(trimmed).escape_debug().to_string();
        return Err(NativeError::NotAVersion { command, shown });
    };
    let reported = match version {
        LUA54_VERSION => Some(LuaAbi::Lua54),
        LUA51_VERSION => Some(LuaAbi::LuaJit),
        _ => None,
    };
    match (request.lua, reported) {
        (None, Some(abi)) => Ok(Package::from(abi)),
        (None, None) => Err(NativeError::Unsupported {
            version: version.to_owned(),
        }),
        (Some(wanted), Some(abi)) if wanted == abi => Ok(Package::from(abi)),
        (Some(wanted), _) => Err(NativeError::Disagrees {
            command,
            version: version.to_owned(),
            wanted: lua_version(wanted),
            flag: wanted.flag_value(),
        }),
    }
}

/// Whether `text` is two runs of ASCII digits around one dot.
fn is_version(text: &str) -> bool {
    text.split_once('.').is_some_and(|(major, minor)| {
        let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
        digits(major) && digits(minor)
    })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::io;

    use super::{Flavour, NativeError, NativeRequest, Package, hint, install_native, setup_native};
    use crate::cli::LuaAbi;
    use crate::lua::luarocks::scripted::Scripted;
    use crate::lua::luarocks::{Captured, LuarocksError, Outcome};

    const VERSION: &str = airsl::VERSION;
    const BOOK_PAGE: &str = "https://airsstack.github.io/airsl/ch01-05-stock-lua.html";
    const BOOK_POINTING: &str = "https://airsstack.github.io/airsl/ch01-05-stock-lua.html#pointing-luarocks-at-lua-54-or-luajit";

    /// What `luarocks config cache` prints when the Lua it targets is `LuaJIT`.
    const LUAJIT_CACHE: &[u8] =
        b"{\n   luajit_version = \"2.1.1770848219\",\n   luajit_version_checked = true\n}\n";

    /// What `luarocks config cache` prints when it found no `LuaJIT` for Lua 5.1: the Lua 5.1 it
    /// targets is not `LuaJIT`, or there is no Lua 5.1 interpreter at all.
    const PUC_CACHE: &[u8] = b"{\n   luajit_version_checked = true\n}\n";

    fn request(lua: Option<LuaAbi>) -> NativeRequest {
        NativeRequest { lua }
    }

    /// A luarocks that targets Lua 5.1 and then answers `config cache` with `cache`.
    fn lua51_with(cache: &[u8]) -> Scripted {
        Scripted::default().then_stdout(b"5.1\n").then_stdout(cache)
    }

    fn detect(stdout: &[u8], lua: Option<LuaAbi>) -> Result<Package, NativeError> {
        let luarocks = Scripted::default().then_stdout(stdout);
        setup_native(&luarocks, request(lua), Flavour::Unix)
    }

    fn refusal(result: Result<Package, NativeError>) -> String {
        result.unwrap_err().to_string()
    }

    fn answered(code: Option<i32>, stdout: &[u8]) -> Scripted {
        Scripted::default().then_query(Ok(Captured {
            stdout: stdout.to_vec(),
            code,
        }))
    }

    #[test]
    fn lua_5_4_chooses_airsstack() {
        assert_eq!(detect(b"5.4\n", None).unwrap(), Package::Airsstack);
        assert_eq!(Package::Airsstack.as_str(), "airsstack");
    }

    #[test]
    fn lua_5_1_that_is_luajit_chooses_airsstack_luajit() {
        let luarocks = lua51_with(LUAJIT_CACHE);

        let package = setup_native(&luarocks, request(None), Flavour::Unix).unwrap();

        assert_eq!(package, Package::AirsstackLuajit);
        assert_eq!(Package::AirsstackLuajit.as_str(), "airsstack-luajit");
        assert_eq!(luarocks.calls(), ["config lua_version", "config cache"]);
    }

    fn no_luajit_refusal(lua_version_flag: &str) -> String {
        format!(
            "`luarocks` found no LuaJIT for Lua 5.1: its Lua 5.1 is not LuaJIT, or it has none, \
             and `airsstack-luajit` needs LuaJIT\n\
             {pad}install Lua 5.4 and run again with `--lua 5.4` to install it for Lua 5.4\n\
             {pad}or run `luarocks {lua_version_flag}config --scope user lua_dir <LuaJIT prefix>` \
             to point `luarocks` at LuaJIT\n\
             {pad}{BOOK_POINTING} shows how to point `luarocks` at Lua 5.4 or LuaJIT",
            pad = "       "
        )
    }

    #[test]
    fn lua_5_1_without_luajit_is_refused_naming_both_ways_out() {
        let luarocks = lua51_with(PUC_CACHE);

        let error = setup_native(&luarocks, request(None), Flavour::Unix);

        assert_eq!(refusal(error), no_luajit_refusal("--lua-version 5.1 "));
        assert_eq!(luarocks.calls(), ["config lua_version", "config cache"]);
    }

    #[test]
    fn luajit_named_with_the_lua_flag_is_refused_the_same_way_without_luajit() {
        let luarocks = Scripted::default()
            .then_stdout(b"5.1\n")
            .then_stdout(PUC_CACHE);

        let error = setup_native(&luarocks, request(Some(LuaAbi::LuaJit)), Flavour::Unix);

        assert_eq!(refusal(error), no_luajit_refusal("--lua-version 5.1 "));
        assert_eq!(
            luarocks.calls(),
            [
                "--lua-version 5.1 config lua_version",
                "--lua-version 5.1 config cache"
            ]
        );
    }

    #[test]
    fn lua_5_4_is_never_asked_whether_it_is_luajit() {
        let luarocks = Scripted::default().then_stdout(b"5.4\n");

        setup_native(&luarocks, request(None), Flavour::Unix).unwrap();

        assert_eq!(luarocks.calls(), ["config lua_version"]);
    }

    #[test]
    fn a_cache_answer_that_cannot_be_read_does_not_stop_a_lua_5_1_install() {
        let unreadable: [Scripted; 4] = [
            Scripted::default().then_stdout(b"5.1\n").then_stdout(b""),
            Scripted::default()
                .then_stdout(b"5.1\n")
                .then_stdout(b"{\n   something_else = true\n}\n"),
            Scripted::default()
                .then_stdout(b"5.1\n")
                .then_query(Ok(Captured {
                    stdout: PUC_CACHE.to_vec(),
                    code: Some(1),
                })),
            Scripted::default()
                .then_stdout(b"5.1\n")
                .then_query(Ok(Captured {
                    stdout: PUC_CACHE.to_vec(),
                    code: None,
                })),
        ];

        for luarocks in unreadable {
            let package = setup_native(&luarocks, request(None), Flavour::Unix).unwrap();

            assert_eq!(package, Package::AirsstackLuajit);
        }
    }

    #[test]
    fn the_luajit_keys_are_read_whole_whatever_the_spacing() {
        // The checked key starts with the version key, so a prefix match would find a version in
        // a table that has none; luarocks prints spaces around `=`, but nothing here depends on it.
        let spaced = b"{\n   luajit_version_checked = true\n}\n";
        let unspaced = b"{\n   luajit_version_checked=true\n}\n";

        for cache in [&spaced[..], &unspaced[..]] {
            let error = setup_native(&lua51_with(cache), request(None), Flavour::Unix);

            assert!(
                matches!(error, Err(NativeError::NoLuajit)),
                "{:?}: {error:?}",
                String::from_utf8_lossy(cache)
            );
        }
    }

    #[test]
    fn a_luajit_table_written_without_spaces_is_still_luajit() {
        let json_like = b"{\n   luajit_version=\"2.1\",\n   luajit_version_checked=true\n}\n";
        let luarocks = lua51_with(json_like);

        let package = setup_native(&luarocks, request(None), Flavour::Unix).unwrap();

        assert_eq!(package, Package::AirsstackLuajit);
    }

    #[test]
    fn a_cache_query_that_cannot_start_is_refused_like_detection() {
        let luarocks = Scripted::default()
            .then_stdout(b"5.1\n")
            .then_query(Err(LuarocksError::Missing));

        let error = setup_native(&luarocks, request(None), Flavour::Unix);

        assert!(
            matches!(error, Err(NativeError::Missing { .. })),
            "{error:?}"
        );
    }

    #[test]
    fn a_padded_or_crlf_version_is_trimmed() {
        assert_eq!(detect(b" 5.4 \r\n", None).unwrap(), Package::Airsstack);
        assert_eq!(detect(b"5.4", None).unwrap(), Package::Airsstack);
    }

    #[test]
    fn an_empty_answer_is_refused_as_no_version() {
        let expected = "`luarocks config lua_version` printed no Lua version";
        for stdout in [&b""[..], b"\n", b"  \t "] {
            assert_eq!(refusal(detect(stdout, None)), expected, "{stdout:?}");
        }
    }

    #[test]
    fn a_three_part_version_is_refused_as_not_a_version() {
        assert_eq!(
            refusal(detect(b"5.4.8\n", None)),
            "`luarocks config lua_version` printed `5.4.8`, which is not a Lua version"
        );
    }

    #[test]
    fn two_lines_are_refused_and_shown_escaped() {
        assert_eq!(
            refusal(detect(b"5.4\n5.1\n", None)),
            "`luarocks config lua_version` printed `5.4\\n5.1`, which is not a Lua version"
        );
    }

    #[test]
    fn non_ascii_whitespace_and_a_bom_are_not_trimmed() {
        let shown = |stdout: &[u8]| refusal(detect(stdout, None));

        assert!(shown("5.4\u{a0}".as_bytes()).contains("`5.4\\u{a0}`"));
        assert!(shown("\u{feff}5.4".as_bytes()).contains("`\\u{feff}5.4`"));
        assert!(shown("５.４".as_bytes()).contains("not a Lua version"));
    }

    #[test]
    fn a_backtick_in_the_answer_is_shown_as_it_is() {
        assert_eq!(
            refusal(detect(b"5.4`x`\n", None)),
            "`luarocks config lua_version` printed `5.4`x``, which is not a Lua version"
        );
    }

    #[test]
    fn the_host_flavour_is_the_one_of_the_operating_system_it_runs_on() {
        #[cfg(windows)]
        assert_eq!(Flavour::host(), Flavour::Windows);
        #[cfg(not(windows))]
        assert_eq!(Flavour::host(), Flavour::Unix);
    }

    #[test]
    fn invalid_utf8_is_shown_lossily() {
        assert!(refusal(detect(b"\xff5.4\n", None)).contains("`\u{fffd}5.4`"));
    }

    #[test]
    fn lua_5_5_is_refused_naming_the_flag_and_the_book_section() {
        let expected = format!(
            "`luarocks` installs for Lua 5.5, and `airsstack` is built for Lua 5.4 and LuaJIT\n\
             {pad}run again with `--lua 5.4` to install it for Lua 5.4\n\
             {pad}{BOOK_POINTING} shows how to point `luarocks` at Lua 5.4 or LuaJIT",
            pad = "       "
        );

        assert_eq!(refusal(detect(b"5.5\n", None)), expected);
        assert!(refusal(detect(b"10.0\n", None)).starts_with("`luarocks` installs for Lua 10.0,"));
    }

    #[test]
    fn lua_5_04_is_refused_as_unsupported_not_read_as_5_4() {
        assert!(refusal(detect(b"5.04\n", None)).starts_with("`luarocks` installs for Lua 5.04,"));
    }

    #[test]
    fn without_the_lua_flag_no_lua_version_is_passed() {
        let luarocks = Scripted::default().then_stdout(b"5.4\n");

        setup_native(&luarocks, request(None), Flavour::Unix).unwrap();

        assert_eq!(luarocks.calls(), ["config lua_version"]);
    }

    #[test]
    fn the_lua_flag_refuses_a_disagreeing_answer() {
        let luajit = refusal(detect(b"5.4\n", Some(LuaAbi::LuaJit)));
        let lua54 = refusal(detect(b"5.1\n", Some(LuaAbi::Lua54)));

        assert_eq!(
            luajit,
            "`luarocks --lua-version 5.1 config lua_version` printed `5.4`, \
             not the 5.1 that `--lua luajit` asks for"
        );
        assert_eq!(
            lua54,
            "`luarocks --lua-version 5.4 config lua_version` printed `5.1`, \
             not the 5.4 that `--lua 5.4` asks for"
        );
    }

    #[test]
    fn the_lua_flag_with_an_unsupported_answer_is_a_disagreement_not_an_unsupported_lua() {
        let error = refusal(detect(b"5.5\n", Some(LuaAbi::Lua54)));

        assert_eq!(
            error,
            "`luarocks --lua-version 5.4 config lua_version` printed `5.5`, \
             not the 5.4 that `--lua 5.4` asks for"
        );
    }

    #[test]
    fn a_config_failure_names_the_command_and_its_status() {
        let error = setup_native(&answered(Some(1), b""), request(None), Flavour::Unix);

        assert_eq!(
            refusal(error),
            "`luarocks config lua_version` failed with exit status 1"
        );
    }

    #[test]
    fn a_nonzero_config_exit_wins_over_a_valid_version_on_stdout() {
        let error = setup_native(&answered(Some(1), b"5.4\n"), request(None), Flavour::Unix);

        assert!(matches!(error, Err(NativeError::Failed { code: 1, .. })));
    }

    #[test]
    fn a_config_signal_is_reported() {
        let error = setup_native(&answered(None, b""), request(None), Flavour::Unix);

        assert_eq!(
            refusal(error),
            "`luarocks config lua_version` was stopped by a signal"
        );
    }

    #[test]
    fn a_config_signal_wins_over_a_valid_version_on_stdout() {
        let error = setup_native(&answered(None, b"5.4\n"), request(None), Flavour::Unix);

        assert!(matches!(error, Err(NativeError::Signal { .. })));
    }

    #[test]
    fn a_spawn_failure_names_luarocks_and_the_io_error() {
        let spawn = |kind: io::ErrorKind, flavour: Flavour| {
            let luarocks = Scripted::default().then_query(Err(LuarocksError::Spawn {
                source: io::Error::new(kind, "the reason"),
            }));
            setup_native(&luarocks, request(None), flavour)
                .unwrap_err()
                .to_string()
        };

        for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::OutOfMemory] {
            assert_eq!(
                spawn(kind, Flavour::Unix),
                "run failed on `luarocks`: the reason"
            );
            assert_eq!(
                spawn(kind, Flavour::Windows),
                "run failed on `luarocks.exe`: the reason"
            );
        }
    }

    fn missing(lua: Option<LuaAbi>, flavour: Flavour) -> String {
        let luarocks = Scripted::default().then_query(Err(LuarocksError::Missing));
        setup_native(&luarocks, request(lua), flavour)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn a_missing_luarocks_prints_both_install_lines() {
        let expected = format!(
            "`luarocks` was not found on PATH\n\
             {pad}install LuaRocks, then run the line for your Lua:\n\
             {pad}luarocks --lua-version 5.4 install --local airsstack {VERSION}\n\
             {pad}luarocks --lua-version 5.1 install --local airsstack-luajit {VERSION}",
            pad = "       "
        );

        assert_eq!(missing(None, Flavour::Unix), expected);
    }

    #[test]
    fn a_missing_luarocks_with_the_lua_flag_prints_one_install_line() {
        assert_eq!(
            missing(Some(LuaAbi::Lua54), Flavour::Unix),
            format!(
                "`luarocks` was not found on PATH\n\
                 {pad}install LuaRocks, then run: \
                 luarocks --lua-version 5.4 install --local airsstack {VERSION}",
                pad = "       "
            )
        );
        assert_eq!(
            missing(Some(LuaAbi::LuaJit), Flavour::Unix),
            format!(
                "`luarocks` was not found on PATH\n\
                 {pad}install LuaRocks, then run: \
                 luarocks --lua-version 5.1 install --local airsstack-luajit {VERSION}",
                pad = "       "
            )
        );
    }

    #[test]
    fn a_missing_luarocks_on_windows_names_the_exe_and_the_bat_wrapper() {
        let wrapper = "       airsl runs `luarocks.exe` itself, not a `luarocks.bat` wrapper";

        assert_eq!(
            missing(None, Flavour::Windows),
            format!(
                "`luarocks.exe` was not found on PATH\n\
                 {wrapper}\n\
                 {pad}install LuaRocks, then run the line for your Lua:\n\
                 {pad}luarocks --lua-version 5.4 install --local airsstack {VERSION}\n\
                 {pad}luarocks --lua-version 5.1 install --local airsstack-luajit {VERSION}",
                pad = "       "
            )
        );
        assert_eq!(
            missing(Some(LuaAbi::LuaJit), Flavour::Windows),
            format!(
                "`luarocks.exe` was not found on PATH\n\
                 {wrapper}\n\
                 {pad}install LuaRocks, then run: \
                 luarocks --lua-version 5.1 install --local airsstack-luajit {VERSION}",
                pad = "       "
            )
        );
    }

    fn installed(code: Option<i32>) -> Scripted {
        Scripted::default().then_run(Ok(Outcome { code }))
    }

    fn install(
        luarocks: &Scripted,
        lua: Option<LuaAbi>,
        package: Package,
    ) -> Result<(), NativeError> {
        install_native(luarocks, request(lua), package, Flavour::Unix)
    }

    #[test]
    fn the_install_arguments_are_local_package_and_the_airsl_version() {
        let luarocks = installed(Some(0));

        install(&luarocks, None, Package::Airsstack).unwrap();

        assert_eq!(
            luarocks.calls(),
            [format!("install --local airsstack {VERSION}")]
        );
    }

    #[test]
    fn the_lua_flag_passes_lua_version_to_every_call() {
        let luarocks = lua51_with(LUAJIT_CACHE).then_run(Ok(Outcome { code: Some(0) }));
        let request = request(Some(LuaAbi::LuaJit));

        let package = setup_native(&luarocks, request, Flavour::Unix).unwrap();
        install_native(&luarocks, request, package, Flavour::Unix).unwrap();

        assert_eq!(
            luarocks.calls(),
            [
                "--lua-version 5.1 config lua_version".to_owned(),
                "--lua-version 5.1 config cache".to_owned(),
                format!("--lua-version 5.1 install --local airsstack-luajit {VERSION}"),
            ]
        );
    }

    #[test]
    fn an_install_failure_exits_one_whatever_luarocks_returned() {
        let error = install(&installed(Some(2)), None, Package::Airsstack).unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("`luarocks install --local airsstack {VERSION}` failed with exit status 2")
        );
    }

    #[test]
    fn an_install_signal_is_reported() {
        let error = install(&installed(None), None, Package::Airsstack).unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("`luarocks install --local airsstack {VERSION}` was stopped by a signal")
        );
    }

    #[test]
    fn an_install_that_cannot_start_is_refused_like_detection() {
        let luarocks = Scripted::default().then_run(Err(LuarocksError::Missing));

        let error = install(&luarocks, Some(LuaAbi::Lua54), Package::Airsstack).unwrap_err();

        assert!(matches!(error, NativeError::Missing { .. }), "{error:?}");
    }

    #[test]
    fn the_hint_names_luarocks_path_for_the_chosen_lua() {
        let hint = |lua| hint(request(lua), Flavour::Unix);

        assert_eq!(
            hint(None),
            r#"require("airsstack") works once you run: eval "$(luarocks path)""#
        );
        assert_eq!(
            hint(Some(LuaAbi::Lua54)),
            r#"require("airsstack") works once you run: eval "$(luarocks --lua-version 5.4 path)""#
        );
        assert_eq!(
            hint(Some(LuaAbi::LuaJit)),
            r#"require("airsstack") works once you run: eval "$(luarocks --lua-version 5.1 path)""#
        );
    }

    #[test]
    fn the_windows_hint_names_the_cmd_and_powershell_forms() {
        let plain = hint(request(None), Flavour::Windows);
        let luajit = hint(request(Some(LuaAbi::LuaJit)), Flavour::Windows);

        assert_eq!(
            plain,
            "require(\"airsstack\") works once luarocks' paths are set. In cmd:\n  \
             luarocks path > lrpath.bat\n  \
             call lrpath.bat\n\
             In PowerShell:\n  \
             $env:LUA_PATH = luarocks path --lr-path\n  \
             $env:LUA_CPATH = luarocks path --lr-cpath"
        );
        assert_eq!(plain.lines().count(), 6);
        assert_eq!(
            hint(request(Some(LuaAbi::Lua54)), Flavour::Windows),
            "require(\"airsstack\") works once luarocks' paths are set. In cmd:\n  \
             luarocks --lua-version 5.4 path > lrpath.bat\n  \
             call lrpath.bat\n\
             In PowerShell:\n  \
             $env:LUA_PATH = luarocks --lua-version 5.4 path --lr-path\n  \
             $env:LUA_CPATH = luarocks --lua-version 5.4 path --lr-cpath"
        );
        assert_eq!(
            luajit,
            "require(\"airsstack\") works once luarocks' paths are set. In cmd:\n  \
             luarocks --lua-version 5.1 path > lrpath.bat\n  \
             call lrpath.bat\n\
             In PowerShell:\n  \
             $env:LUA_PATH = luarocks --lua-version 5.1 path --lr-path\n  \
             $env:LUA_CPATH = luarocks --lua-version 5.1 path --lr-cpath"
        );
    }

    #[test]
    fn the_rock_version_is_the_cli_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
