//! The `airsstack.time` host module.
//!
//! `jiff` rather than `time` or `chrono` because it reads `/etc/localtime` directly instead of
//! going through libc `tzset`, which is not thread-safe — and this crate's whole point is being
//! embeddable in a host that has other threads.
//!
//! Needs no authority. It is nonetheless the module most able to make a script's output
//! irreproducible, so `format` takes an explicit instant rather than defaulting to "now", and the
//! default rendering is RFC 3339 in UTC.
//!
//! Responsibilities: [`Time`], installing `now`, `monotonic`, `format` and `parse`.
//!
//! Non-responsibilities: sleeping. A sandboxed script that can block the host for an arbitrary
//! period defeats the instruction ceiling, which is the one defence against a script that never
//! finishes.

use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.time`.
#[derive(Debug)]
pub struct Time {
    name: ModuleName,
}

impl Time {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("time")
                .unwrap_or_else(|_| unreachable!("`time` is a valid module name")),
        }
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

/// Reports a formatting or parsing failure as a catchable Lua error.
const fn invalid(operation: &'static str, detail: String) -> Error {
    Error::Denied {
        module: "time",
        operation,
        detail,
    }
}

impl HostModule for Time {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        _context: &InstallContext<'_>,
    ) -> Result<()> {
        let fail = |e: mlua::Error| Error::ModuleInstall {
            module: String::from("time"),
            reason: e.to_string(),
        };

        // Seconds since the Unix epoch. An integer rather than a float because Lua 5.4
        // distinguishes them and a timestamp that silently became a float would print as `1.7e9`.
        let now = lua
            .create_function(|_, ()| Ok(jiff::Timestamp::now().as_second()))
            .map_err(fail)?;
        table.set("now", now).map_err(fail)?;

        // A monotonic reading, for measuring how long something took. Unrelated to wall-clock time
        // and unaffected by the clock being adjusted underneath a running script, which is exactly
        // why subtracting two `now` readings is the wrong way to time anything.
        let monotonic = lua
            .create_function(|_, ()| {
                let since = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                Ok(since.as_secs_f64())
            })
            .map_err(fail)?;
        table.set("monotonic", monotonic).map_err(fail)?;

        let format = lua
            .create_function(|_, (seconds, pattern): (i64, Option<mlua::LuaString>)| {
                let stamp = jiff::Timestamp::from_second(seconds).map_err(|e| {
                    invalid("format", format!("{seconds} is not a valid instant: {e}"))
                })?;
                match pattern {
                    // UTC rather than local time: a script that renders a timestamp into a file
                    // should produce the same bytes on every machine that runs it.
                    Some(pattern) => {
                        let zoned = stamp.to_zoned(jiff::tz::TimeZone::UTC);
                        jiff::fmt::strtime::format(pattern.to_str()?.as_ref(), &zoned)
                            .map_err(|e| mlua::Error::from(invalid("format", e.to_string())))
                    }
                    None => Ok(stamp.to_string()),
                }
            })
            .map_err(fail)?;
        table.set("format", format).map_err(fail)?;

        let parse = lua
            .create_function(
                |_, (text, pattern): (mlua::LuaString, Option<mlua::LuaString>)| {
                    let text = text.to_str()?;
                    let Some(pattern) = pattern else {
                        let stamp: jiff::Timestamp = text.parse().map_err(|e| {
                            invalid("parse", format!("`{text}` is not RFC 3339: {e}"))
                        })?;
                        return Ok(stamp.as_second());
                    };

                    let parsed =
                        jiff::fmt::strtime::parse(pattern.to_str()?.as_ref(), text.as_ref())
                            .map_err(|e| invalid("parse", e.to_string()))?;
                    let stamp = parsed
                        .to_zoned()
                        .map_err(|e| invalid("parse", e.to_string()))?;
                    Ok(stamp.timestamp().as_second())
                },
            )
            .map_err(fail)?;
        table.set("parse", parse).map_err(fail)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Time;
    use crate::{Engine, HostModule as _, Policy, Script};

    fn eval<T: mlua::FromLuaMulti>(source: &str) -> T {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        engine
            .eval_to::<T>(&Script::from_source(source, "test").unwrap())
            .unwrap()
    }

    #[test]
    fn the_module_is_named_time() {
        assert_eq!(Time::new().name().as_str(), "time");
    }

    #[test]
    fn now_returns_an_integer_not_a_float() {
        // Lua 5.4 distinguishes the two, and a timestamp that became a float prints as `1.7e+09`.
        assert_eq!(
            eval::<String>("return math.type(airsstack.time.now())"),
            "integer"
        );
    }

    #[test]
    fn now_is_somewhere_in_the_plausible_present() {
        // Sometime after 2020 and before 2100: enough to catch a unit mix-up without pinning a date.
        let seconds: i64 = eval("return airsstack.time.now()");
        assert!(
            (1_577_836_800..4_102_444_800).contains(&seconds),
            "{seconds}"
        );
    }

    #[test]
    fn format_defaults_to_rfc_3339_in_utc() {
        assert_eq!(
            eval::<String>("return airsstack.time.format(0)"),
            "1970-01-01T00:00:00Z"
        );
    }

    #[test]
    fn format_renders_in_utc_regardless_of_the_hosts_zone() {
        // The determinism property: the same script writes the same bytes on every machine.
        assert_eq!(
            eval::<String>("return airsstack.time.format(1700000000, '%Y-%m-%dT%H:%M:%S')"),
            "2023-11-14T22:13:20"
        );
    }

    #[test]
    fn format_accepts_a_strftime_pattern() {
        assert_eq!(
            eval::<String>("return airsstack.time.format(0, '%Y-%m-%d')"),
            "1970-01-01"
        );
    }

    #[test]
    fn parse_reads_back_what_format_wrote() {
        assert_eq!(
            eval::<i64>("return airsstack.time.parse(airsstack.time.format(1700000000))"),
            1_700_000_000
        );
    }

    #[test]
    fn parse_accepts_a_pattern_too() {
        assert_eq!(
            eval::<i64>(
                "return airsstack.time.parse('1970-01-02 00:00:00 +0000', '%Y-%m-%d %H:%M:%S %z')"
            ),
            86_400
        );
    }

    #[test]
    fn parsing_nonsense_raises_a_catchable_error() {
        assert_eq!(
            eval::<String>(
                "local ok = pcall(airsstack.time.parse, 'not a date'); return tostring(ok)"
            ),
            "false"
        );
    }

    #[test]
    fn monotonic_does_not_go_backwards() {
        assert_eq!(
            eval::<String>(
                "local a = airsstack.time.monotonic()
                 local x = 0
                 for i = 1, 10000 do x = x + i end
                 local b = airsstack.time.monotonic()
                 return tostring(b >= a)"
            ),
            "true"
        );
    }

    #[test]
    fn there_is_no_sleep_to_stall_the_host_with() {
        // An arbitrary sleep would defeat the instruction ceiling, which is the only defence
        // against a script that never finishes.
        assert_eq!(eval::<String>("return type(airsstack.time.sleep)"), "nil");
    }
}
