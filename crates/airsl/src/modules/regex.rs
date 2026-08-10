//! The `airsstack.regex` host module.
//!
//! Exists because Lua patterns are not regular expressions and the difference bites exactly where
//! it matters: no alternation, no `\b`, no non-greedy repetition, no named groups. A script that
//! needs any of those under raw Lua ends up shelling out, which is the situation this crate exists
//! to remove.
//!
//! Needs no authority — matching text reaches nothing — so it is installed under every preset.
//!
//! Responsibilities: [`Regex`], installing the one-shot functions and `compile`.
//!
//! Non-responsibilities: reading the subject from anywhere. Every function takes the text.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.regex`.
#[derive(Debug)]
pub struct Regex {
    name: ModuleName,
}

impl Regex {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("regex")
                .unwrap_or_else(|_| unreachable!("`regex` is a valid module name")),
        }
    }
}

impl Default for Regex {
    fn default() -> Self {
        Self::new()
    }
}

/// Compiles `pattern`, reporting a bad one as a catchable Lua error.
fn compile(pattern: &str) -> Result<regex::Regex> {
    regex::Regex::new(pattern).map_err(|source| Error::Denied {
        module: "regex",
        operation: "compile",
        detail: format!("`{pattern}` is not a valid regular expression: {source}"),
    })
}

/// The captures of `matched` as a Lua table: `[0]` whole match, `[n]` groups, names as keys.
fn captures_table(
    lua: &mlua::Lua,
    re: &regex::Regex,
    caught: &regex::Captures<'_>,
) -> mlua::Result<mlua::Table> {
    let out = lua.create_table()?;
    for (index, group) in caught.iter().enumerate() {
        if let Some(found) = group {
            out.set(index, found.as_str())?;
        }
    }
    // Named groups are set alongside the numbers rather than instead of them, so a pattern can be
    // read either way without the caller knowing which style it was written in.
    for name in re.capture_names().flatten() {
        if let Some(found) = caught.name(name) {
            out.set(name, found.as_str())?;
        }
    }
    Ok(out)
}

/// Builds the table `compile` hands back: the same operations, bound to one compiled pattern.
fn compiled_table(lua: &mlua::Lua, re: regex::Regex) -> mlua::Result<mlua::Table> {
    let out = lua.create_table()?;
    let re = Arc::new(re);

    let r = Arc::clone(&re);
    out.set(
        "is_match",
        lua.create_function(move |_, text: mlua::LuaString| Ok(r.is_match(&text.to_str()?)))?,
    )?;

    let r = Arc::clone(&re);
    out.set(
        "find",
        lua.create_function(move |_, text: mlua::LuaString| {
            Ok(r.find(&text.to_str()?).map(|m| m.as_str().to_owned()))
        })?,
    )?;

    let r = Arc::clone(&re);
    out.set(
        "find_all",
        lua.create_function(move |lua, text: mlua::LuaString| {
            let text = text.to_str()?;
            lua.create_sequence_from(r.find_iter(&text).map(|m| m.as_str().to_owned()))
        })?,
    )?;

    let r = Arc::clone(&re);
    out.set(
        "captures",
        lua.create_function(move |lua, text: mlua::LuaString| {
            let text = text.to_str()?;
            r.captures(&text)
                .map(|caught| captures_table(lua, &r, &caught))
                .transpose()
        })?,
    )?;

    let r = Arc::clone(&re);
    out.set(
        "replace",
        lua.create_function(move |_, (text, with): (mlua::LuaString, mlua::LuaString)| {
            Ok(r.replace(&text.to_str()?, with.to_str()?.as_ref())
                .into_owned())
        })?,
    )?;

    let r = Arc::clone(&re);
    out.set(
        "replace_all",
        lua.create_function(move |_, (text, with): (mlua::LuaString, mlua::LuaString)| {
            Ok(r.replace_all(&text.to_str()?, with.to_str()?.as_ref())
                .into_owned())
        })?,
    )?;

    out.set(
        "split",
        lua.create_function(move |lua, text: mlua::LuaString| {
            let text = text.to_str()?;
            lua.create_sequence_from(re.split(&text).map(ToOwned::to_owned))
        })?,
    )?;

    Ok(out)
}

impl HostModule for Regex {
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
            module: String::from("regex"),
            reason: e.to_string(),
        };

        // `compile` exists because the one-shot forms recompile on every call, and the plugin
        // scripts match the same pattern over thousands of lines.
        let compiled = lua
            .create_function(|lua, pattern: mlua::LuaString| {
                let re = compile(&pattern.to_str()?)?;
                compiled_table(lua, re)
            })
            .map_err(fail)?;
        table.set("compile", compiled).map_err(fail)?;

        let is_match = lua
            .create_function(|_, (pattern, text): (mlua::LuaString, mlua::LuaString)| {
                Ok(compile(&pattern.to_str()?)?.is_match(&text.to_str()?))
            })
            .map_err(fail)?;
        table.set("is_match", is_match).map_err(fail)?;

        let find = lua
            .create_function(|_, (pattern, text): (mlua::LuaString, mlua::LuaString)| {
                Ok(compile(&pattern.to_str()?)?
                    .find(&text.to_str()?)
                    .map(|m| m.as_str().to_owned()))
            })
            .map_err(fail)?;
        table.set("find", find).map_err(fail)?;

        let find_all = lua
            .create_function(|lua, (pattern, text): (mlua::LuaString, mlua::LuaString)| {
                let re = compile(&pattern.to_str()?)?;
                let text = text.to_str()?;
                lua.create_sequence_from(re.find_iter(&text).map(|m| m.as_str().to_owned()))
            })
            .map_err(fail)?;
        table.set("find_all", find_all).map_err(fail)?;

        let captures = lua
            .create_function(|lua, (pattern, text): (mlua::LuaString, mlua::LuaString)| {
                let re = compile(&pattern.to_str()?)?;
                let text = text.to_str()?;
                re.captures(&text)
                    .map(|caught| captures_table(lua, &re, &caught))
                    .transpose()
            })
            .map_err(fail)?;
        table.set("captures", captures).map_err(fail)?;

        let replace = lua
            .create_function(
                |_, (pattern, text, with): (mlua::LuaString, mlua::LuaString, mlua::LuaString)| {
                    Ok(compile(&pattern.to_str()?)?
                        .replace(&text.to_str()?, with.to_str()?.as_ref())
                        .into_owned())
                },
            )
            .map_err(fail)?;
        table.set("replace", replace).map_err(fail)?;

        let replace_all = lua
            .create_function(
                |_, (pattern, text, with): (mlua::LuaString, mlua::LuaString, mlua::LuaString)| {
                    Ok(compile(&pattern.to_str()?)?
                        .replace_all(&text.to_str()?, with.to_str()?.as_ref())
                        .into_owned())
                },
            )
            .map_err(fail)?;
        table.set("replace_all", replace_all).map_err(fail)?;

        let split = lua
            .create_function(|lua, (pattern, text): (mlua::LuaString, mlua::LuaString)| {
                let re = compile(&pattern.to_str()?)?;
                let text = text.to_str()?;
                lua.create_sequence_from(re.split(&text).map(ToOwned::to_owned))
            })
            .map_err(fail)?;
        table.set("split", split).map_err(fail)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Regex;
    use crate::{Engine, HostModule as _, Policy, Script};

    fn eval(source: &str) -> String {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        engine
            .eval_to::<String>(&Script::from_source(source, "test").unwrap())
            .unwrap()
    }

    #[test]
    fn the_module_is_named_regex() {
        assert_eq!(Regex::new().name().as_str(), "regex");
    }

    #[test]
    fn is_match_answers_yes_and_no() {
        assert_eq!(
            eval("return tostring(airsstack.regex.is_match('^a.c$', 'abc'))"),
            "true"
        );
        assert_eq!(
            eval("return tostring(airsstack.regex.is_match('^a.c$', 'abd'))"),
            "false"
        );
    }

    #[test]
    fn alternation_works_where_a_lua_pattern_would_not() {
        // The reason this module exists: Lua patterns have no `|`.
        assert_eq!(
            eval("return tostring(airsstack.regex.is_match('^(cat|dog)$', 'dog'))"),
            "true"
        );
    }

    #[test]
    fn a_word_boundary_works_where_a_lua_pattern_would_not() {
        assert_eq!(
            eval(r"return tostring(airsstack.regex.is_match([[\bcat\b]], 'the cat sat'))"),
            "true"
        );
        assert_eq!(
            eval(r"return tostring(airsstack.regex.is_match([[\bcat\b]], 'concatenate'))"),
            "false"
        );
    }

    #[test]
    fn find_returns_the_first_match_and_find_all_returns_every_one() {
        assert_eq!(
            eval(r"return airsstack.regex.find([[\d+]], 'a12b34')"),
            "12"
        );
        assert_eq!(
            eval(r"return table.concat(airsstack.regex.find_all([[\d+]], 'a12b34'), ',')"),
            "12,34"
        );
    }

    #[test]
    fn find_returns_nil_when_nothing_matches() {
        assert_eq!(
            eval(r"return type(airsstack.regex.find([[\d+]], 'abc'))"),
            "nil"
        );
    }

    #[test]
    fn captures_are_reachable_by_number() {
        assert_eq!(
            eval(
                r"local c = airsstack.regex.captures([[(\w+)@(\w+)]], 'me@here')
                   return c[0] .. '|' .. c[1] .. '|' .. c[2]"
            ),
            "me@here|me|here"
        );
    }

    #[test]
    fn captures_are_also_reachable_by_name() {
        assert_eq!(
            eval(
                r"local c = airsstack.regex.captures([[(?<user>\w+)@(?<host>\w+)]], 'me@here')
                   return c.user .. '|' .. c.host"
            ),
            "me|here"
        );
    }

    #[test]
    fn captures_returns_nil_when_nothing_matches() {
        assert_eq!(
            eval(r"return type(airsstack.regex.captures([[(\d+)]], 'abc'))"),
            "nil"
        );
    }

    #[test]
    fn replace_changes_the_first_and_replace_all_changes_every_one() {
        assert_eq!(
            eval(r"return airsstack.regex.replace([[\d]], 'a1b2', 'X')"),
            "aXb2"
        );
        assert_eq!(
            eval(r"return airsstack.regex.replace_all([[\d]], 'a1b2', 'X')"),
            "aXbX"
        );
    }

    #[test]
    fn a_replacement_can_refer_to_a_capture() {
        assert_eq!(
            eval(r"return airsstack.regex.replace_all([[(\w+)@(\w+)]], 'me@here', '$2:$1')"),
            "here:me"
        );
    }

    #[test]
    fn split_breaks_on_every_match() {
        assert_eq!(
            eval(r"return table.concat(airsstack.regex.split([[\s*,\s*]], 'a , b,c'), '|')"),
            "a|b|c"
        );
    }

    #[test]
    fn an_invalid_pattern_raises_a_catchable_error_naming_it() {
        let out = eval(
            "local ok, err = pcall(airsstack.regex.is_match, '(unclosed', 'x')
             return tostring(ok) .. ':' .. tostring(err):match('not a valid') ",
        );
        assert_eq!(out, "false:not a valid");
    }

    #[test]
    fn a_compiled_pattern_offers_the_same_operations() {
        assert_eq!(
            eval(
                r"local re = airsstack.regex.compile([[\d+]])
                   return tostring(re.is_match('a1')) .. ',' .. re.find('a12b')
                       .. ',' .. table.concat(re.find_all('a1b2'), '')
                       .. ',' .. re.replace_all('a1b2', 'X')
                       .. ',' .. table.concat(re.split('a1b'), '|')"
            ),
            "true,12,12,aXbX,a|b"
        );
    }

    #[test]
    fn a_compiled_pattern_still_captures_by_name() {
        assert_eq!(
            eval(
                r"local re = airsstack.regex.compile([[(?<n>\d+)]])
                   return re.captures('a12').n"
            ),
            "12"
        );
    }

    #[test]
    fn the_module_needs_no_grant_so_a_pure_policy_has_all_of_it() {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let script = Script::from_source(
            "local names = {'compile','is_match','find','find_all','captures',
                            'replace','replace_all','split'}
             for _, n in ipairs(names) do
               if type(airsstack.regex[n]) ~= 'function' then return n end
             end
             return 'all'",
            "probe",
        )
        .unwrap();
        assert_eq!(engine.eval_to::<String>(&script).unwrap(), "all");
    }
}
