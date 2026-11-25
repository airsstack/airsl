//! What a script is allowed to reach, and what it may consume doing so.
//!
//! Exists as one type composing three because the three axes are genuinely independent. A script
//! may need the full language surface and a tight memory ceiling, or a minimal surface and no
//! ceiling at all, and a single enum over the combinations would have to enumerate their product
//! to say any of it.
//!
//! Responsibilities:
//!
//! - [`Policy`], composing the language surface, the grants and the resource limits.
//! - The three presets, which are what most callers should use.
//!
//! Non-responsibilities: applying any of it. [`crate::EngineBuilder`] reads a policy while building
//! the state; nothing here touches the VM.

use crate::sandbox::grant_set::GrantSet;
use crate::sandbox::language_surface::LanguageSurface;
use crate::sandbox::resource_limits::{InstructionLimit, MemoryLimit, ResourceLimits};

/// Memory ceiling the confined preset imposes.
const CONFINED_MEMORY: MemoryLimit = MemoryLimit::mebibytes(64);

/// Instruction ceiling the confined preset imposes.
const CONFINED_INSTRUCTIONS: InstructionLimit = InstructionLimit::count(100_000_000);

/// Memory ceiling the pure preset imposes.
const PURE_MEMORY: MemoryLimit = MemoryLimit::mebibytes(16);

/// Instruction ceiling the pure preset imposes.
const PURE_INSTRUCTIONS: InstructionLimit = InstructionLimit::count(10_000_000);

/// What a script may reach, and what it may spend.
///
/// Build one from a preset and adjust it if needed. There is no builder: once the presets exist a
/// policy has no field that must be chosen, so every combination is one wither away.
///
/// # Examples
///
/// ```
/// use airsl::{Policy, ResourceLimits};
///
/// let policy = Policy::confined();
/// assert!(policy.limits().memory().is_some());
///
/// let unbounded = Policy::confined().with_limits(ResourceLimits::none());
/// assert!(unbounded.limits().memory().is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    language: LanguageSurface,
    grants: GrantSet,
    limits: ResourceLimits,
}

impl Policy {
    /// Every safe Lua library, unrestricted grants, no ceilings.
    ///
    /// For first-party code trusted to the same degree as the host. A script under this policy can
    /// read and write arbitrary files and spawn processes without going through a host module.
    #[must_use]
    pub const fn trusted() -> Self {
        Self {
            language: LanguageSurface::Full,
            grants: GrantSet::unrestricted(),
            limits: ResourceLimits::none(),
        }
    }

    /// The default: a restricted language surface, declared grants only, and both ceilings.
    ///
    /// What a script written by someone other than the host author should run under.
    #[must_use]
    pub const fn confined() -> Self {
        Self {
            language: LanguageSurface::Restricted,
            grants: GrantSet::declared(),
            limits: ResourceLimits::new(Some(CONFINED_MEMORY), Some(CONFINED_INSTRUCTIONS)),
        }
    }

    /// A minimal language surface, a sealed grant set, and tight ceilings.
    ///
    /// For evaluating configuration, expressions and generated snippets. This is the configuration
    /// whose guarantees are strongest and easiest to state, which makes it the right target for
    /// adversarial testing.
    ///
    /// The grants are [`GrantSet::sealed`]: no [`HostAccess`](crate::HostAccess) member and no
    /// allowlist entry. A host widening this policy builds on `GrantSet::sealed()`, not on
    /// `GrantSet::declared()`, which holds every member and so reopens what `pure` closes.
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            language: LanguageSurface::Minimal,
            grants: GrantSet::sealed(),
            limits: ResourceLimits::new(Some(PURE_MEMORY), Some(PURE_INSTRUCTIONS)),
        }
    }

    /// The same policy on a different language surface.
    #[must_use]
    pub const fn with_language(mut self, language: LanguageSurface) -> Self {
        self.language = language;
        self
    }

    /// The same policy with different grants.
    ///
    /// Not `const`, unlike its siblings: a [`GrantSet`] owns its allowlists, so replacing one
    /// drops the old value and a const context cannot run a destructor.
    #[must_use]
    pub fn with_grants(mut self, grants: GrantSet) -> Self {
        self.grants = grants;
        self
    }

    /// The same policy with different ceilings.
    #[must_use]
    pub const fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The language surface this policy selects.
    #[must_use]
    pub const fn language(&self) -> LanguageSurface {
        self.language
    }

    /// The grants this policy extends.
    #[must_use]
    pub const fn grants(&self) -> &GrantSet {
        &self.grants
    }

    /// The ceilings this policy imposes.
    #[must_use]
    pub const fn limits(&self) -> &ResourceLimits {
        &self.limits
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::confined()
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        clippy::panic,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::collections::BTreeMap;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

    use super::Policy;
    use crate::engine::Engine;
    use crate::sandbox::grant_set::GrantSet;
    use crate::sandbox::host_access::HostAccess;
    use crate::sandbox::language_surface::LanguageSurface;
    use crate::sandbox::resource_limits::{MemoryLimit, ResourceLimits};
    use crate::script::Script;
    use crate::test_support::abs;

    #[test]
    fn confined_is_the_default_policy() {
        assert_eq!(Policy::default(), Policy::confined());
    }

    #[test]
    fn trusted_waives_the_surface_the_grants_and_the_ceilings() {
        let policy = Policy::trusted();
        assert_eq!(policy.language(), LanguageSurface::Full);
        assert!(policy.grants().is_unrestricted());
        assert_eq!(*policy.limits(), ResourceLimits::none());
    }

    #[test]
    fn confined_restricts_the_surface_and_imposes_both_ceilings() {
        let policy = Policy::confined();
        assert_eq!(policy.language(), LanguageSurface::Restricted);
        assert_eq!(
            policy.grants().access().collect::<Vec<_>>(),
            HostAccess::ALL
        );
        assert!(policy.limits().memory().is_some());
        assert!(policy.limits().instructions().is_some());
    }

    #[test]
    fn pure_is_tighter_than_confined_on_every_axis_that_can_be() {
        let pure = Policy::pure();
        let confined = Policy::confined();
        assert_eq!(pure.language(), LanguageSurface::Minimal);
        assert!(pure.grants().is_empty());
        for member in HostAccess::ALL {
            assert!(!pure.grants().allows(member), "{member}");
        }
        assert!(pure.limits().memory() < confined.limits().memory());
        assert!(pure.limits().instructions() < confined.limits().instructions());
    }

    #[test]
    fn each_wither_replaces_one_axis_and_leaves_the_others() {
        let policy = Policy::confined().with_language(LanguageSurface::Minimal);
        assert_eq!(policy.language(), LanguageSurface::Minimal);
        assert_eq!(policy.limits(), Policy::confined().limits());

        let policy = Policy::confined().with_grants(GrantSet::unrestricted());
        assert!(policy.grants().is_unrestricted());
        assert_eq!(policy.language(), LanguageSurface::Restricted);

        let policy = Policy::confined().with_limits(ResourceLimits::none());
        assert_eq!(*policy.limits(), ResourceLimits::none());
        assert_eq!(policy.language(), LanguageSurface::Restricted);
    }

    #[test]
    fn a_policy_can_be_tightened_beyond_any_preset() {
        let policy = Policy::pure()
            .with_limits(ResourceLimits::none().with_memory(Some(MemoryLimit::bytes(1))));
        assert_eq!(policy.limits().memory().map(MemoryLimit::get), Some(1));
    }

    /// Runs `body` as a file script in a fresh temp dir, so the script has a root, and returns
    /// the string-to-string table it returns. `siblings` are written next to it first.
    fn run_table(
        policy: Policy,
        body: &str,
        siblings: &[(&str, &str)],
    ) -> BTreeMap<String, String> {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in siblings {
            std::fs::write(dir.path().join(name), text).unwrap();
        }
        std::fs::write(dir.path().join("main.lua"), body).unwrap();
        let script = Script::from_file(dir.path().join("main.lua")).unwrap();
        let engine = Engine::builder().policy(policy).build().unwrap();
        engine.eval_to(&script).unwrap()
    }

    /// How long [`run_table_unless_stalled`] waits before it treats the evaluation as blocked.
    const STALL_LIMIT: Duration = Duration::from_secs(10);

    /// [`run_table`] on a spawned thread, failing instead of hanging when it does not return.
    ///
    /// A call that should have been refused but instead reads standard input would block the
    /// test on the terminal; waiting with a deadline turns that into a failure. The blocked thread
    /// is left behind and ends with the test process.
    fn run_table_unless_stalled(policy: Policy, body: String) -> BTreeMap<String, String> {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            // The receiver is gone only after the deadline passed, when nobody wants the result.
            let _ = sender.send(run_table(policy, &body, &[]));
        });
        match receiver.recv_timeout(STALL_LIMIT) {
            Ok(table) => table,
            Err(RecvTimeoutError::Timeout) => panic!(
                "a stdin-reading call was not refused: the evaluation was still running after \
                 {STALL_LIMIT:?}"
            ),
            Err(RecvTimeoutError::Disconnected) => {
                panic!("the evaluation thread ended without returning a table")
            }
        }
    }

    /// The prelude both scripts open with: `try`, which returns a call's refusal text or
    /// `NO ERROR`; `s`, the host-module table; and `out`, the table the script returns.
    const TRY: &str = "local function try(f, ...) \
        local ok, e = pcall(f, ...) \
        if ok then return 'NO ERROR' end \
        return tostring(e) \
    end \
    local s = airsstack \
    local out = {} ";

    /// The first `math.random(1, 1000000)` of a fresh `pure` engine.
    fn pure_first_random() -> i64 {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let script = Script::from_source("return math.random(1, 1000000)", "random").unwrap();
        engine.eval_to(&script).unwrap()
    }

    /// Every call a script could use to reach something outside the VM, each wrapped so the
    /// refusal text comes back instead of aborting the script.
    fn pure_probe_script() -> String {
        format!(
            "{TRY} \
             out.print = tostring(print) \
             out.stdio_read = try(s.stdio.read) \
             out.stdio_lines = try(s.stdio.lines) \
             out.stdio_write = try(s.stdio.write, 'x') \
             out.stdio_error = try(s.stdio.error, 'x') \
             out.stdio_isatty = try(s.stdio.isatty) \
             out.hook_payload = try(s.hook.payload) \
             out.hook_emit = try(s.hook.emit, 'e', {{}}) \
             out.hook_context = try(s.hook.context, 'Stop', 'x') \
             out.time_now = try(s.time.now) \
             out.time_monotonic = try(s.time.monotonic) \
             out.time_parse = try(s.time.parse, '2024-01-01 America/New_York', '%Y-%m-%d %Q') \
             out.path_absolute = try(s.path.absolute, 'x') \
             out.fs_read_relative = try(s.fs.read, 'p.lua') \
             out.fs_read_absolute = try(s.fs.read, '{absolute}') \
             out.hash_file = try(s.hash.hash_file, 'p.lua') \
             out.glob_walk = try(s.glob.walk, '.', '*.lua') \
             out.require = try(require, 'x') \
             out.randomseed = try(math.randomseed) \
             return out",
            absolute = abs("/etc/hosts"),
        )
    }

    /// The exact refusal for each probe but `fs_read_absolute`, whose text carries a path spelled
    /// per platform.
    const PURE_REFUSALS: [(&str, &str); 18] = [
        ("print", "nil"),
        (
            "stdio_read",
            "stdio.read denied: `stdin` is not granted — no access is granted",
        ),
        (
            "stdio_lines",
            "stdio.lines denied: `stdin` is not granted — no access is granted",
        ),
        (
            "stdio_write",
            "stdio.write denied: `stdout` is not granted — no access is granted",
        ),
        (
            "stdio_error",
            "stdio.error denied: `stderr` is not granted — no access is granted",
        ),
        (
            "stdio_isatty",
            "stdio.isatty denied: `stdout` is not granted — no access is granted",
        ),
        (
            "hook_payload",
            "hook.payload denied: `stdin` is not granted — no access is granted",
        ),
        (
            "hook_emit",
            "hook.emit denied: `stdout` is not granted — no access is granted",
        ),
        (
            "hook_context",
            "hook.context denied: `stdout` is not granted — no access is granted",
        ),
        (
            "time_now",
            "time.now denied: `clock` is not granted — no access is granted",
        ),
        (
            "time_monotonic",
            "time.monotonic denied: `clock` is not granted — no access is granted",
        ),
        (
            "time_parse",
            "time.parse denied: the named time zone `America/New_York` needs `zones`, \
             which is not granted — no access is granted",
        ),
        (
            "path_absolute",
            "path.absolute denied: the relative path `x` needs `cwd`, \
             which is not granted — no access is granted",
        ),
        (
            "fs_read_relative",
            "fs.read denied: `p.lua` is outside the granted read roots: none are granted",
        ),
        (
            "hash_file",
            "hash.hash_file denied: `p.lua` is outside the granted read roots: \
             none are granted",
        ),
        (
            "glob_walk",
            "glob.walk denied: `.` is outside the granted read roots: none are granted",
        ),
        (
            "require",
            "require denied: `require` is not granted — no access is granted",
        ),
        (
            "randomseed",
            "math.randomseed denied: a seed is required — the no-argument form reads the clock",
        ),
    ];

    #[test]
    fn pure_reaches_nothing_a_script_could_observe() {
        let found = run_table_unless_stalled(Policy::pure(), pure_probe_script());

        for (key, text) in PURE_REFUSALS {
            let got = found.get(key).map(String::as_str);
            assert!(
                got.and_then(|got| got.lines().next()) == Some(text),
                "{key}: expected `{text}`, got {got:?}"
            );
        }
        let absolute_refusal = format!(
            "fs.read denied: `{}` is outside the granted read roots: none are granted",
            abs("/etc/hosts")
        );
        assert!(
            found["fs_read_absolute"].lines().next() == Some(absolute_refusal.as_str()),
            "{}",
            found["fs_read_absolute"]
        );
        assert_eq!(found.len(), PURE_REFUSALS.len() + 1);

        let first = pure_first_random();
        assert_eq!(first, pure_first_random());
        assert_eq!(first, 275_394);
    }

    #[test]
    fn confined_keeps_every_host_access_member() {
        let body = format!(
            "{TRY} \
             out.access = table.concat(s.ext.granted().access, ',') \
             out.isatty_stdin = try(s.stdio.isatty, 'stdin') \
             out.isatty_stdout = try(s.stdio.isatty, 'stdout') \
             out.isatty_stderr = try(s.stdio.isatty, 'stderr') \
             out.time_now = try(s.time.now) \
             out.time_monotonic = try(s.time.monotonic) \
             out.time_parse = try(s.time.parse, '2024-01-01 America/New_York', '%Y-%m-%d %Q') \
             out.path_absolute = try(s.path.absolute, 'x') \
             out.require = try(require, 'sibling') \
             return out"
        );
        let found = run_table(Policy::confined(), &body, &[("sibling.lua", "return 42")]);

        assert_eq!(
            found["access"],
            "stdin,stdout,stderr,clock,cwd,require,zones"
        );
        for key in [
            "isatty_stdin",
            "isatty_stdout",
            "isatty_stderr",
            "time_now",
            "time_monotonic",
            "path_absolute",
        ] {
            assert_eq!(found[key], "NO ERROR", "{key}");
        }
        assert_eq!(found["require"], "NO ERROR");
        // Every `time.parse` failure is a `Denied`, so only the refusal's own words tell it apart
        // from a host whose tz database lacks the zone.
        assert!(
            !found["time_parse"].contains("needs `zones`"),
            "{}",
            found["time_parse"]
        );
    }
}
