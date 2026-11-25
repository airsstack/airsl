//! A Lua type expression a host uses to describe a value, and its spelling in the Lua language
//! server's annotation syntax.
//!
//! Its own module because the type is the one piece of a description every other piece mentions: a
//! parameter, a return, a record field and a callback all carry one. Keeping the representation
//! in a private field here means the only way to build a type is through the constructors, so a
//! union is always flattened and an optional is never doubled. The crate reads it back through
//! `LuaType::repr`.
//!
//! Responsibilities:
//!
//! - [`LuaType`], its primitive constants and its constructors.
//! - The normalisation `or` and `optional` perform, and the spelling that follows from it.
//!
//! Non-responsibilities: validating anything. A literal, a record name and a parameter name are
//! stored as given; [`ModuleDescriptionBuilder::build`] checks them.
//!
//! [`ModuleDescriptionBuilder::build`]: super::ModuleDescriptionBuilder::build

#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use super::Signature;

/// A Lua type, built only through the associated constants and constructors below.
///
/// The representation is private. That keeps two invariants out of the host's hands: a union
/// holds no nested union and no optional member, and an optional never wraps another optional.
///
/// # Examples
///
/// ```
/// use airsl::description::LuaType;
///
/// let algorithm = LuaType::literal("sha256").or(LuaType::literal("sha1"));
/// let paths = LuaType::optional(LuaType::array(LuaType::STRING));
/// # let _ = (algorithm, paths);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaType(Repr);

/// The shape of a [`LuaType`], read by [`ModuleDescriptionBuilder::build`] to walk a type and by
/// the file renderer. Any module in the crate can name a variant, but a [`LuaType`] wraps one in
/// a private field, so only its constructors can make a type that holds one.
///
/// [`ModuleDescriptionBuilder::build`]: super::ModuleDescriptionBuilder::build
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Repr {
    Primitive(&'static str),
    Literal(String),
    Array(Box<LuaType>),
    Map(Box<LuaType>, Box<LuaType>),
    /// Never wraps another `Optional`.
    Optional(Box<LuaType>),
    Function(Signature),
    Record(String),
    /// At least two members, none of them a `Union` or an `Optional`.
    Union(Vec<LuaType>),
}

impl LuaType {
    /// The `nil` type.
    pub const NIL: Self = Self(Repr::Primitive("nil"));
    /// The `boolean` type.
    pub const BOOLEAN: Self = Self(Repr::Primitive("boolean"));
    /// The `integer` type.
    pub const INTEGER: Self = Self(Repr::Primitive("integer"));
    /// The `number` type.
    pub const NUMBER: Self = Self(Repr::Primitive("number"));
    /// The `string` type.
    pub const STRING: Self = Self(Repr::Primitive("string"));
    /// The `any` type.
    pub const ANY: Self = Self(Repr::Primitive("any"));
    /// The `table` type, for a table whose shape is not described.
    pub const TABLE: Self = Self(Repr::Primitive("table"));

    /// A string literal type, such as `"sha1"`. The empty string is allowed.
    ///
    /// The text is stored as given and not checked here.
    #[must_use]
    pub fn literal(text: &str) -> Self {
        Self(Repr::Literal(text.to_owned()))
    }

    /// A sequence of `element`, spelled `element[]`.
    #[must_use]
    pub fn array(element: Self) -> Self {
        Self(Repr::Array(Box::new(element)))
    }

    /// A table from `key` to `value`, spelled `table<key, value>`.
    #[must_use]
    pub fn map(key: Self, value: Self) -> Self {
        Self(Repr::Map(Box::new(key), Box::new(value)))
    }

    /// `ty` or `nil`, spelled `ty?`.
    ///
    /// Idempotent: the optional of an optional is the same optional.
    #[must_use]
    pub fn optional(ty: Self) -> Self {
        if matches!(ty.0, Repr::Optional(_)) {
            ty
        } else {
            Self(Repr::Optional(Box::new(ty)))
        }
    }

    /// A function type, spelled `fun(...)`.
    #[must_use]
    pub const fn function(signature: Signature) -> Self {
        Self(Repr::Function(signature))
    }

    /// A record declared by the same module description, named by its unqualified name.
    ///
    /// It spells as `<root>.<module>.<name>` once the module it belongs to is known. The name is
    /// stored as given and not checked here.
    #[must_use]
    pub fn record(name: &str) -> Self {
        Self(Repr::Record(name.to_owned()))
    }

    /// The union of `self` and `other`, spelled `a|b`.
    ///
    /// Nested unions flatten into one member list in left-to-right order. An optional member makes
    /// the whole union optional, so `?` never appears inside a union.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        let mut members = Vec::new();
        let left_optional = self.absorb_into(&mut members);
        let right_optional = other.absorb_into(&mut members);
        let union = Self(Repr::Union(members));
        if left_optional || right_optional {
            Self(Repr::Optional(Box::new(union)))
        } else {
            union
        }
    }

    pub(crate) const fn repr(&self) -> &Repr {
        &self.0
    }

    /// Appends this type's union members to `members`, returning whether it was optional.
    fn absorb_into(self, members: &mut Vec<Self>) -> bool {
        match self.0 {
            Repr::Optional(inner) => {
                inner.absorb_into(members);
                true
            }
            Repr::Union(own) => {
                members.extend(own);
                false
            }
            other => {
                members.push(Self(other));
                false
            }
        }
    }

    /// The type's text in the Lua language server's annotation syntax.
    ///
    /// `prefix` is `<root>.<module>`, the qualifier a record name takes.
    pub(crate) fn spell(&self, prefix: &str) -> String {
        match &self.0 {
            Repr::Primitive(name) => (*name).to_owned(),
            Repr::Literal(text) => format!("\"{text}\""),
            Repr::Array(element) => {
                let inner = element.spell(prefix);
                if matches!(
                    element.0,
                    Repr::Union(_) | Repr::Optional(_) | Repr::Function(_)
                ) {
                    format!("({inner})[]")
                } else {
                    format!("{inner}[]")
                }
            }
            Repr::Map(key, value) => {
                format!("table<{}, {}>", key.spell(prefix), value.spell(prefix))
            }
            Repr::Optional(inner) => {
                let spelled = inner.spell(prefix);
                if matches!(inner.0, Repr::Union(_) | Repr::Function(_)) {
                    format!("({spelled})?")
                } else {
                    format!("{spelled}?")
                }
            }
            Repr::Function(signature) => signature.spell(prefix),
            Repr::Record(name) => format!("{prefix}.{name}"),
            Repr::Union(members) => members
                .iter()
                .map(|member| {
                    let spelled = member.spell(prefix);
                    // A bare `fun(...): r|other` reads as a function returning the union.
                    if matches!(member.0, Repr::Function(_)) {
                        format!("({spelled})")
                    } else {
                        spelled
                    }
                })
                .collect::<Vec<_>>()
                .join("|"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LuaType;
    use crate::description::Signature;

    const BOOLEAN: LuaType = LuaType::BOOLEAN;
    const INTEGER: LuaType = LuaType::INTEGER;
    const STRING: LuaType = LuaType::STRING;

    fn spell(ty: &LuaType) -> String {
        ty.spell("airsstack.fs")
    }

    fn predicate() -> LuaType {
        LuaType::function(Signature::new().param("x", STRING).returns(BOOLEAN))
    }

    #[test]
    fn primitives_spell_as_their_lua_names() {
        let spelled: Vec<String> = [
            LuaType::NIL,
            LuaType::BOOLEAN,
            LuaType::INTEGER,
            LuaType::NUMBER,
            LuaType::STRING,
            LuaType::ANY,
            LuaType::TABLE,
        ]
        .iter()
        .map(spell)
        .collect();
        assert_eq!(
            spelled,
            [
                "nil", "boolean", "integer", "number", "string", "any", "table"
            ]
        );
    }

    #[test]
    fn a_literal_spells_quoted() {
        assert_eq!(spell(&LuaType::literal("sha1")), "\"sha1\"");
    }

    #[test]
    fn a_union_joins_members_with_a_bar() {
        assert_eq!(spell(&STRING.or(INTEGER)), "string|integer");
    }

    #[test]
    fn an_array_appends_brackets() {
        assert_eq!(spell(&LuaType::array(STRING)), "string[]");
        assert_eq!(
            spell(&LuaType::array(STRING.or(INTEGER))),
            "(string|integer)[]"
        );
    }

    #[test]
    fn a_map_spells_as_table_of_key_and_value() {
        assert_eq!(
            spell(&LuaType::map(STRING, STRING)),
            "table<string, string>"
        );
        assert_eq!(
            spell(&LuaType::optional(LuaType::map(INTEGER.or(STRING), STRING))),
            "table<integer|string, string>?"
        );
    }

    #[test]
    fn an_optional_appends_a_question_mark() {
        assert_eq!(
            spell(&LuaType::optional(STRING.or(INTEGER))),
            "(string|integer)?"
        );
        assert_eq!(
            spell(&LuaType::optional(predicate())),
            "(fun(x: string): boolean)?"
        );
        assert_eq!(spell(&LuaType::optional(STRING)), "string?");
    }

    #[test]
    fn a_function_spells_params_and_returns() {
        let full = LuaType::function(
            Signature::new()
                .param("a", STRING)
                .optional("b", INTEGER)
                .variadic(LuaType::NUMBER)
                .returns(STRING)
                .returns(INTEGER),
        );
        assert_eq!(
            spell(&full),
            "fun(a: string, b?: integer, ...: number): string, integer"
        );

        let no_returns =
            LuaType::function(Signature::new().param("a", STRING).optional("b", INTEGER));
        assert_eq!(spell(&no_returns), "fun(a: string, b?: integer)");
    }

    #[test]
    fn a_record_spells_with_the_module_prefix() {
        assert_eq!(spell(&LuaType::record("Stat")), "airsstack.fs.Stat");
    }

    #[test]
    fn optional_of_an_optional_is_one_optional() {
        let twice = LuaType::optional(LuaType::optional(STRING));
        assert_eq!(twice, LuaType::optional(STRING));
        assert_eq!(spell(&twice), "string?");
    }

    #[test]
    fn a_function_in_a_union_is_parenthesised() {
        assert_eq!(
            spell(&predicate().or(STRING)),
            "(fun(x: string): boolean)|string"
        );
    }

    #[test]
    fn an_array_of_an_optional_is_parenthesised() {
        assert_eq!(
            spell(&LuaType::array(LuaType::optional(STRING))),
            "(string?)[]"
        );
        assert_eq!(
            spell(&LuaType::array(predicate())),
            "(fun(x: string): boolean)[]"
        );
    }

    #[test]
    fn an_optional_array_needs_no_parentheses() {
        assert_eq!(
            spell(&LuaType::optional(LuaType::array(STRING))),
            "string[]?"
        );
        assert_eq!(
            spell(&LuaType::array(LuaType::map(STRING, STRING))),
            "table<string, string>[]"
        );
    }

    #[test]
    fn a_union_of_unions_is_flattened() {
        assert_eq!(
            spell(&STRING.or(INTEGER.or(BOOLEAN))),
            "string|integer|boolean"
        );
        assert_eq!(
            spell(&STRING.or(INTEGER).or(BOOLEAN)),
            "string|integer|boolean"
        );
        // The spelling alone cannot tell a nested union from a flat one, so the values are
        // compared too.
        assert_eq!(
            STRING.or(INTEGER.or(BOOLEAN)),
            STRING.or(INTEGER).or(BOOLEAN)
        );
        assert_eq!(
            LuaType::optional(STRING.or(INTEGER)).or(BOOLEAN),
            STRING.or(INTEGER).or(LuaType::optional(BOOLEAN))
        );
    }

    #[test]
    fn an_optional_member_makes_the_whole_union_optional() {
        assert_eq!(
            spell(&STRING.or(LuaType::optional(INTEGER))),
            "(string|integer)?"
        );
        assert_eq!(
            spell(&LuaType::optional(STRING).or(INTEGER)),
            "(string|integer)?"
        );
    }

    #[test]
    fn an_empty_literal_spells_as_two_quotes() {
        assert_eq!(spell(&LuaType::literal("")), "\"\"");
        assert_eq!(spell(&LuaType::optional(LuaType::literal("a"))), "\"a\"?");
    }
}
