//! The `airsstack.time` host module.
//!
//! `jiff` rather than `time` or `chrono` because it never goes through libc `tzset`, which is not
//! thread-safe — and this crate's whole point is being embeddable in a host that has other
//! threads. On unix it reads `/etc/localtime` directly; on Windows, where there is no
//! `/etc/localtime`, it carries its own tzdb.
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

use std::sync::OnceLock;
use std::time::Instant;

use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// The instant every `monotonic` reading is measured from.
///
/// Process-wide rather than a field on [`Time`] because a module is installed per engine, and two
/// engines carrying two origins would hand a script readings it had no way to subtract. An
/// [`Instant`] has no epoch, so an origin has to be chosen by somebody; choosing it once here is
/// what makes every reading taken anywhere in the process comparable with every other.
static ORIGIN: OnceLock<Instant> = OnceLock::new();

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

        // Seconds since `ORIGIN`, for measuring how long something took.
        //
        // `Instant` and not `SystemTime`: this is unaffected by the wall clock being adjusted
        // underneath a running script — which is the entire reason to reach for it rather than
        // subtracting two `now` readings. `SystemTime` here would be `now` under a second name,
        // and an NTP step between two readings would make the later one smaller, handing a script
        // a negative duration or a timeout that never elapses. `Instant` reaches the platform's own
        // monotonic clock on every target this crate builds for; that property, not any one
        // platform's name for the mechanism, is what this module depends on.
        //
        // No datetime crate offers this. A `DateTime` is a point on a calendar and a calendar
        // point is defined by the wall clock, so `jiff` and `chrono` both bottom out in
        // `SystemTime::now`; monotonic time has no epoch and no timezone, which is why it lives in
        // `std` as its own type.
        let monotonic = lua
            .create_function(|_, ()| Ok(ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64()))
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
    fn monotonic_is_measured_from_process_start_rather_than_the_unix_epoch() {
        // This is the assertion the loop above cannot make. The wall clock does not go backwards
        // across 10,000 iterations either, so "non-decreasing" holds just as well under an
        // implementation carrying no monotonic guarantee at all — which is what this module used
        // to ship. The magnitude is what tells the two apart.
        //
        // The bound is absurd on purpose: 1e6 seconds is eleven days of uptime, so it cannot fire
        // on a long-lived host, while a reading taken from `SystemTime` is around 1.7e9.
        let reading = eval::<f64>("return airsstack.time.monotonic()");
        assert!(
            (0.0..1.0e6).contains(&reading),
            "expected seconds since process start, got {reading}"
        );
    }

    #[test]
    fn two_engines_take_their_readings_from_one_origin() {
        // A module is installed per engine, so an origin held on `Time` would start the second
        // engine counting again and hand back a smaller number than the first — two clocks
        // wearing one name, and a script subtracting across them would get a negative duration.
        let first = eval::<f64>("return airsstack.time.monotonic()");
        let second = eval::<f64>("return airsstack.time.monotonic()");
        assert!(second >= first, "{second} is behind {first}");
    }

    #[test]
    fn there_is_no_sleep_to_stall_the_host_with() {
        // An arbitrary sleep would defeat the instruction ceiling, which is the only defence
        // against a script that never finishes.
        assert_eq!(eval::<String>("return type(airsstack.time.sleep)"), "nil");
    }
}
