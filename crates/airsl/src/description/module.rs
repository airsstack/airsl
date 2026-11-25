//! A module's whole description: its doc, the functions and fields it exposes, and the records its
//! signatures name. The one place a description is validated.
//!
//! Its own module because every other piece of the description is built from raw text and may be
//! wrong in ways only the whole can show: two members with one name, a record a signature names
//! but nobody described, a doc the language server would read as syntax. Checking all of it in one
//! walk keeps a single, ordered answer to "what is the first problem".
//!
//! Responsibilities:
//!
//! - [`ModuleDescription`] and [`ModuleDescriptionBuilder`], which collects members and records in
//!   the order they were added.
//! - `build`, the validation: names, duplicates, docs, literals, the positions a type may take and
//!   the references between records and the types that name them.
//!
//! Non-responsibilities: spelling a type, rendering a file, and checking that a description matches
//! the keys a module really installs.

#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::collections::HashSet;

use super::lua_type::Repr;
use super::{Field, Function, LuaType, Param, Record, Signature};
use crate::error::{Error, Result};
use crate::types::LuaName;

/// A non-function value a module exposes, such as a constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleField {
    name: String,
    ty: LuaType,
    doc: String,
}

impl ModuleField {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn ty(&self) -> &LuaType {
        &self.ty
    }

    pub(crate) fn doc(&self) -> &str {
        &self.doc
    }
}

/// One key a module installs: a function, or a value that is not a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Member {
    /// A function the module exposes.
    Function(Function),
    /// A value the module exposes.
    Field(ModuleField),
}

impl Member {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Function(function) => function.name(),
            Self::Field(field) => field.name(),
        }
    }
}

/// A validated description of the Lua surface one module exposes.
///
/// Built only through [`ModuleDescription::builder`], so a value of this type has passed every
/// rule `build` enforces. Hosts build descriptions and hand them on; they never read one back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDescription {
    doc: String,
    members: Vec<Member>,
    records: Vec<Record>,
}

impl ModuleDescription {
    /// Starts a description of a module documented by `doc`.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::description::{Function, LuaType, ModuleDescription};
    ///
    /// let description = ModuleDescription::builder("SHA-256 and SHA-1 digests.")
    ///     .function(
    ///         Function::new("hash_file", "Hashes a file's bytes.")
    ///             .param("path", LuaType::STRING, "File to hash.")
    ///             .optional(
    ///                 "algorithm",
    ///                 LuaType::literal("sha256").or(LuaType::literal("sha1")),
    ///                 "Defaults to sha256.",
    ///             )
    ///             .returns(LuaType::STRING),
    ///     )
    ///     .build()?;
    /// # let _ = description;
    /// # Ok::<(), airsl::Error>(())
    /// ```
    #[must_use]
    pub fn builder(doc: &str) -> ModuleDescriptionBuilder {
        ModuleDescriptionBuilder {
            doc: doc.to_owned(),
            members: Vec::new(),
            records: Vec::new(),
        }
    }

    pub(crate) fn doc(&self) -> &str {
        &self.doc
    }

    /// The functions and fields in the order they were added.
    pub(crate) fn members(&self) -> &[Member] {
        &self.members
    }

    /// The records in the order they were added.
    pub(crate) fn records(&self) -> &[Record] {
        &self.records
    }
}

/// Collects the parts of a [`ModuleDescription`]. Holds raw text; [`build`](Self::build) checks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDescriptionBuilder {
    doc: String,
    members: Vec<Member>,
    records: Vec<Record>,
}

impl ModuleDescriptionBuilder {
    /// Adds a function.
    #[must_use]
    pub fn function(mut self, function: Function) -> Self {
        self.members.push(Member::Function(function));
        self
    }

    /// Adds a value that is not a function, such as a constant.
    ///
    /// A function is described with [`function`](Self::function) instead, so a field whose type is
    /// a function type is refused by `build`.
    #[must_use]
    pub fn field(mut self, name: &str, ty: LuaType, doc: &str) -> Self {
        self.members.push(Member::Field(ModuleField {
            name: name.to_owned(),
            ty,
            doc: doc.to_owned(),
        }));
        self
    }

    /// Adds a record a signature can name with [`LuaType::record`].
    #[must_use]
    pub fn record(mut self, record: Record) -> Self {
        self.records.push(record);
        self
    }

    /// Checks everything the builder was given and returns the description.
    ///
    /// Walks in a fixed order and returns the first problem: the module doc, each member in the
    /// order added, each record in the order added, then the cross-checks between members, records
    /// and the records a type names. A required parameter after an optional one and a record
    /// nothing refers to are accepted.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when a function, parameter, field or record name is not a
    /// usable Lua identifier. Returns [`Error::InvalidDescription`] for every other problem: a
    /// duplicate name, a second variadic parameter, a parameter after the variadic one, a doc with
    /// a carriage return or a line starting with `@`, a multi-line doc where one line is required,
    /// a string literal that cannot be written as one, a function-typed module field, a type
    /// that does not fit inside a function type or a map, and a record that is named but not
    /// described.
    pub fn build(self) -> Result<ModuleDescription> {
        check_doc(&self.doc, "the module", DocShape::Free)?;
        let mut member_names = HashSet::new();
        for member in &self.members {
            check_member(member, &mut member_names)?;
        }
        let mut record_names = HashSet::new();
        for record in &self.records {
            check_record(record, &mut record_names)?;
        }
        check_member_record_clashes(&self.records, &member_names)?;
        check_record_references(&self.members, &self.records, &record_names)?;
        Ok(ModuleDescription {
            doc: self.doc,
            members: self.members,
            records: self.records,
        })
    }
}

/// How many lines a doc may span.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DocShape {
    /// Any number of lines.
    Free,
    /// A single line, because the doc is written after `# ` on one annotation line.
    OneLine,
}

/// What the type being walked sits inside, which decides the positions it may take.
#[derive(Clone, Copy)]
enum Level<'a> {
    /// The signature of a described function. Each type is owned by its own parameter, variadic
    /// or return value, and is not restricted by position.
    Described,
    /// A function type nested in a type owned by the named owner. Its parameter, variadic and
    /// return types are restricted, and the owner is what a refusal names.
    Nested(&'a str),
}

/// The characters this crate treats as blank before a doc line's first character, so that
/// indenting an `@` does not get past the refusal of a line that starts with one.
const DOC_BLANKS: [char; 4] = [' ', '\t', '\u{b}', '\u{c}'];

const fn invalid(reason: String) -> Error {
    Error::InvalidDescription { reason }
}

fn check_name(raw: &str) -> Result<()> {
    LuaName::new(raw).map(drop)
}

fn check_doc(doc: &str, owner: &str, shape: DocShape) -> Result<()> {
    if doc.contains('\r') {
        return Err(invalid(format!(
            "the doc of {owner} must not contain a carriage return"
        )));
    }
    if doc
        .split('\n')
        .any(|line| line.trim_start_matches(DOC_BLANKS).starts_with('@'))
    {
        return Err(invalid(format!(
            "a line in the doc of {owner} starts with `@`"
        )));
    }
    if shape == DocShape::OneLine && doc.contains('\n') {
        return Err(invalid(format!("the doc of {owner} must be one line")));
    }
    Ok(())
}

fn check_member<'a>(member: &'a Member, seen: &mut HashSet<&'a str>) -> Result<()> {
    check_name(member.name())?;
    if !seen.insert(member.name()) {
        return Err(invalid(format!("`{}` is described twice", member.name())));
    }
    match member {
        Member::Function(function) => check_function(function),
        Member::Field(field) => check_module_field(field),
    }
}

fn check_function(function: &Function) -> Result<()> {
    let label = format!("`{}`", function.name());
    check_doc(function.doc(), &format!("function {label}"), DocShape::Free)?;
    check_signature(
        function.signature(),
        function.variadic_doc(),
        &label,
        Level::Described,
    )
}

fn check_module_field(field: &ModuleField) -> Result<()> {
    let owner = format!("field `{}`", field.name());
    check_doc(field.doc(), &owner, DocShape::OneLine)?;
    let is_function = match field.ty().repr() {
        Repr::Function(_) => true,
        Repr::Optional(inner) => matches!(inner.repr(), Repr::Function(_)),
        _ => false,
    };
    if is_function {
        return Err(invalid(format!(
            "{owner} has a function type; describe it with `function` instead"
        )));
    }
    check_type(field.ty(), &owner, &format!("`{}`", field.name()))
}

/// Checks a parameter list, its variadic tail and its returns.
///
/// `function` is how a message names the function the signature belongs to, such as `` `run` ``.
fn check_signature(
    signature: &Signature,
    variadic_doc: &str,
    function: &str,
    level: Level<'_>,
) -> Result<()> {
    let mut seen = HashSet::new();
    for param in signature.params() {
        let owner = format!("parameter `{}` of {function}", param.name());
        check_param(param, &owner, &mut seen, function)?;
        check_slot(param.ty(), &owner, function, level)?;
    }
    if signature.variadic_calls() > 1 {
        return Err(invalid(format!("{function} has two variadic parameters")));
    }
    if let Some(tail) = signature.variadic_type() {
        let owner = format!("the variadic parameter of {function}");
        check_doc(variadic_doc, &owner, DocShape::OneLine)?;
        check_slot(tail, &owner, function, level)?;
    }
    let returned = format!("return value of {function}");
    for ty in signature.returns_types() {
        check_slot(ty, &returned, function, level)?;
    }
    Ok(())
}

fn check_param<'a>(
    param: &'a Param,
    owner: &str,
    seen: &mut HashSet<&'a str>,
    function: &str,
) -> Result<()> {
    check_name(param.name())?;
    if !seen.insert(param.name()) {
        return Err(invalid(format!(
            "parameter `{}` of {function} is described twice",
            param.name()
        )));
    }
    if param.follows_variadic() {
        return Err(invalid(format!(
            "parameter `{}` of {function} comes after the variadic parameter",
            param.name()
        )));
    }
    check_doc(param.doc(), owner, DocShape::OneLine)
}

/// Checks a type held by a parameter, variadic or return value whose own owner is `owner`.
///
/// Inside a function type the type is owned by the outermost type's owner instead, and must also
/// fit where `fun(...)` allows it.
fn check_slot(ty: &LuaType, owner: &str, function: &str, level: Level<'_>) -> Result<()> {
    match level {
        Level::Described => check_type(ty, owner, function),
        Level::Nested(outer) => {
            check_nested_position(ty, outer)?;
            check_type(ty, outer, function)
        }
    }
}

/// A parameter or return type of a function type must not be one the Lua language server would
/// misread there.
fn check_nested_position(ty: &LuaType, owner: &str) -> Result<()> {
    let misread =
        matches!(ty.repr(), Repr::Function(_) | Repr::Union(_)) || ty.spell("").starts_with('(');
    if misread {
        return Err(invalid(format!(
            "a function type in {owner} must not hold a function, a union or a parenthesised type as a parameter or return type"
        )));
    }
    Ok(())
}

/// Walks a type depth-first, left to right.
///
/// `owner` is what a refusal names, and `function` names the function a nested signature's
/// parameters belong to.
fn check_type(ty: &LuaType, owner: &str, function: &str) -> Result<()> {
    match ty.repr() {
        Repr::Primitive(_) | Repr::Record(_) => Ok(()),
        Repr::Literal(text) => check_literal(text),
        Repr::Array(element) | Repr::Optional(element) => check_type(element, owner, function),
        Repr::Union(members) => members
            .iter()
            .try_for_each(|member| check_type(member, owner, function)),
        Repr::Map(key, value) => {
            if contains_function(key) || contains_function(value) {
                return Err(invalid(format!(
                    "a map type in {owner} must not hold a function type"
                )));
            }
            check_type(key, owner, function)?;
            check_type(value, owner, function)
        }
        Repr::Function(signature) => check_signature(signature, "", function, Level::Nested(owner)),
    }
}

fn check_literal(text: &str) -> Result<()> {
    if text.contains(['"', '\\', '\n', '\r']) {
        return Err(invalid(format!(
            "string literal type `{}` must not contain a double quote, a backslash or a line break",
            text.escape_debug()
        )));
    }
    Ok(())
}

fn contains_function(ty: &LuaType) -> bool {
    match ty.repr() {
        Repr::Function(_) => true,
        Repr::Primitive(_) | Repr::Literal(_) | Repr::Record(_) => false,
        Repr::Array(element) | Repr::Optional(element) => contains_function(element),
        Repr::Map(key, value) => contains_function(key) || contains_function(value),
        Repr::Union(members) => members.iter().any(contains_function),
    }
}

fn check_record<'a>(record: &'a Record, seen: &mut HashSet<&'a str>) -> Result<()> {
    check_name(record.name())?;
    if !seen.insert(record.name()) {
        return Err(invalid(format!(
            "record `{}` is described twice",
            record.name()
        )));
    }
    check_doc(
        record.doc(),
        &format!("record `{}`", record.name()),
        DocShape::Free,
    )?;
    let mut fields = HashSet::new();
    for field in record.fields() {
        check_record_field(record, field, &mut fields)?;
    }
    Ok(())
}

fn check_record_field<'a>(
    record: &Record,
    field: &'a Field,
    seen: &mut HashSet<&'a str>,
) -> Result<()> {
    let owner = format!("field `{}` of record `{}`", field.name(), record.name());
    check_name(field.name())?;
    if !seen.insert(field.name()) {
        return Err(invalid(format!("{owner} is described twice")));
    }
    check_doc(field.doc(), &owner, DocShape::OneLine)?;
    check_type(field.ty(), &owner, &owner)
}

fn check_member_record_clashes(records: &[Record], members: &HashSet<&str>) -> Result<()> {
    if let Some(record) = records
        .iter()
        .find(|record| members.contains(record.name()))
    {
        return Err(invalid(format!(
            "`{}` is described both as a member and as a record",
            record.name()
        )));
    }
    Ok(())
}

fn check_record_references(
    members: &[Member],
    records: &[Record],
    known: &HashSet<&str>,
) -> Result<()> {
    let member_types = members.iter().flat_map(|member| match member {
        Member::Function(function) => signature_types(function.signature()),
        Member::Field(field) => vec![field.ty()],
    });
    let field_types = records
        .iter()
        .flat_map(|record| record.fields().iter().map(Field::ty));
    if let Some(name) = member_types
        .chain(field_types)
        .find_map(|ty| first_unknown_record(ty, known))
    {
        return Err(invalid(format!(
            "record `{name}` is used but not described"
        )));
    }
    Ok(())
}

/// A signature's types in walk order: parameters, then the variadic tail, then returns.
fn signature_types(signature: &Signature) -> Vec<&LuaType> {
    signature
        .params()
        .iter()
        .map(Param::ty)
        .chain(signature.variadic_type())
        .chain(signature.returns_types())
        .collect()
}

fn first_unknown_record<'a>(ty: &'a LuaType, known: &HashSet<&str>) -> Option<&'a str> {
    match ty.repr() {
        Repr::Primitive(_) | Repr::Literal(_) => None,
        Repr::Record(name) => (!known.contains(name.as_str())).then_some(name.as_str()),
        Repr::Array(element) | Repr::Optional(element) => first_unknown_record(element, known),
        Repr::Map(key, value) => {
            first_unknown_record(key, known).or_else(|| first_unknown_record(value, known))
        }
        Repr::Union(members) => members
            .iter()
            .find_map(|member| first_unknown_record(member, known)),
        Repr::Function(signature) => signature_types(signature)
            .into_iter()
            .find_map(|inner| first_unknown_record(inner, known)),
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap fixtures that are valid by construction"
    )]

    use super::{Member, ModuleDescription, ModuleDescriptionBuilder};
    use crate::description::{Field, Function, LuaType, Record, Signature};

    const STRING: LuaType = LuaType::STRING;
    const INTEGER: LuaType = LuaType::INTEGER;
    const ANY: LuaType = LuaType::ANY;

    const PREFIX: &str = "invalid module description: ";

    /// The full `Display` of the error `build` returns, failing the test if it builds.
    fn refusal(builder: ModuleDescriptionBuilder) -> String {
        builder.build().unwrap_err().to_string()
    }

    fn described(reason: &str) -> String {
        format!("{PREFIX}{reason}")
    }

    fn callback(signature: Signature) -> LuaType {
        LuaType::function(signature)
    }

    fn empty() -> ModuleDescriptionBuilder {
        ModuleDescription::builder("")
    }

    #[test]
    fn a_valid_description_builds() {
        let description = ModuleDescription::builder("SHA-256 and SHA-1 digests.")
            .function(
                Function::new("hash_file", "Hashes a file's bytes.")
                    .param("path", STRING, "File to hash.")
                    .optional(
                        "algorithm",
                        LuaType::literal("sha256").or(LuaType::literal("sha1")),
                        "Defaults to sha256.",
                    )
                    .returns(STRING),
            )
            .function(
                Function::new("sha256", "SHA-256 of a string.")
                    .param("data", STRING, "Bytes to hash.")
                    .returns(STRING),
            )
            .field("version", INTEGER, "Digest format version.")
            .record(Record::new("Digest", "A digest.").field("hex", STRING, "Lowercase hex."))
            .build()
            .unwrap();
        assert_eq!(description.doc(), "SHA-256 and SHA-1 digests.");
        let names: Vec<&str> = description.members().iter().map(Member::name).collect();
        assert_eq!(names, ["hash_file", "sha256", "version"]);
        assert!(matches!(
            description.members().first(),
            Some(Member::Function(_))
        ));
        assert!(matches!(
            description.members().get(2),
            Some(Member::Field(_))
        ));
        let records: Vec<&str> = description.records().iter().map(Record::name).collect();
        assert_eq!(records, ["Digest"]);
    }

    #[test]
    fn a_member_described_twice_is_refused() {
        let builder = empty()
            .function(Function::new("f", ""))
            .field("f", STRING, "");
        assert_eq!(refusal(builder), described("`f` is described twice"));
    }

    #[test]
    fn a_record_sharing_a_member_name_is_refused() {
        let builder = empty()
            .function(Function::new("Stat", ""))
            .record(Record::new("Stat", ""));
        assert_eq!(
            refusal(builder),
            described("`Stat` is described both as a member and as a record")
        );
    }

    #[test]
    fn a_parameter_described_twice_is_refused() {
        let builder = empty().function(
            Function::new("f", "")
                .param("p", STRING, "")
                .optional("p", STRING, ""),
        );
        assert_eq!(
            refusal(builder),
            described("parameter `p` of `f` is described twice")
        );
    }

    #[test]
    fn two_variadics_are_refused() {
        let builder = empty().function(Function::new("f", "").variadic(ANY, "").variadic(ANY, ""));
        assert_eq!(
            refusal(builder),
            described("`f` has two variadic parameters")
        );
    }

    #[test]
    fn a_parameter_after_the_variadic_is_refused() {
        let builder = empty().function(
            Function::new("f", "")
                .variadic(ANY, "")
                .param("p", STRING, ""),
        );
        assert_eq!(
            refusal(builder),
            described("parameter `p` of `f` comes after the variadic parameter")
        );
    }

    #[test]
    fn a_record_described_twice_is_refused() {
        let builder = empty()
            .record(Record::new("R", ""))
            .record(Record::new("R", ""));
        assert_eq!(refusal(builder), described("record `R` is described twice"));
    }

    #[test]
    fn a_record_field_described_twice_is_refused() {
        let builder = empty().record(
            Record::new("R", "")
                .field("x", STRING, "")
                .field("x", INTEGER, ""),
        );
        assert_eq!(
            refusal(builder),
            described("field `x` of record `R` is described twice")
        );
    }

    #[test]
    fn an_undescribed_record_reference_is_refused() {
        let builder = empty().function(Function::new("f", "").returns(LuaType::record("Nope")));
        assert_eq!(
            refusal(builder),
            described("record `Nope` is used but not described")
        );
    }

    fn undescribed() -> String {
        described("record `Nope` is used but not described")
    }

    #[test]
    fn an_undescribed_record_in_a_record_field_is_refused() {
        let builder = empty().record(Record::new("R", "").field("f", LuaType::record("Nope"), ""));
        assert_eq!(refusal(builder), undescribed());
    }

    #[test]
    fn an_undescribed_record_in_a_module_field_is_refused() {
        let builder = empty().field("last", LuaType::record("Nope"), "");
        assert_eq!(refusal(builder), undescribed());
    }

    #[test]
    fn an_undescribed_record_in_a_nested_function_type_is_refused() {
        let handler = callback(Signature::new().param("a", LuaType::record("Nope")));
        let builder = empty().function(Function::new("f", "").param("cb", handler, ""));
        assert_eq!(refusal(builder), undescribed());
    }

    #[test]
    fn an_undescribed_record_in_a_map_is_refused() {
        for map in [
            LuaType::map(STRING, LuaType::record("Nope")),
            LuaType::map(LuaType::record("Nope"), STRING),
        ] {
            let builder = empty().function(Function::new("f", "").returns(map));
            assert_eq!(refusal(builder), undescribed());
        }
    }

    #[test]
    fn an_undescribed_record_in_a_union_is_refused() {
        let union = STRING.or(LuaType::record("Nope"));
        let builder = empty().function(Function::new("f", "").returns(union));
        assert_eq!(refusal(builder), undescribed());
    }

    #[test]
    fn a_described_record_reference_is_accepted() {
        let built = empty()
            .function(
                Function::new("f", "")
                    .param(
                        "cb",
                        callback(Signature::new().returns(LuaType::record("R"))),
                        "",
                    )
                    .returns(LuaType::optional(LuaType::array(LuaType::record("R")))),
            )
            .record(Record::new("R", "").field("self", LuaType::record("R"), ""))
            .build();
        assert!(built.is_ok());
    }

    #[test]
    fn a_carriage_return_in_a_doc_is_refused() {
        let cr = "a\rb";
        let cases: [(ModuleDescriptionBuilder, &str); 7] = [
            (ModuleDescription::builder(cr), "the module"),
            (empty().function(Function::new("f", cr)), "function `f`"),
            (
                empty().function(Function::new("f", "").param("p", STRING, cr)),
                "parameter `p` of `f`",
            ),
            (
                empty().function(Function::new("f", "").variadic(ANY, cr)),
                "the variadic parameter of `f`",
            ),
            (empty().field("x", STRING, cr), "field `x`"),
            (empty().record(Record::new("R", cr)), "record `R`"),
            (
                empty().record(Record::new("R", "").field("x", STRING, cr)),
                "field `x` of record `R`",
            ),
        ];
        for (builder, owner) in cases {
            assert_eq!(
                refusal(builder),
                described(&format!(
                    "the doc of {owner} must not contain a carriage return"
                ))
            );
        }
    }

    #[test]
    fn a_doc_line_starting_with_at_is_refused() {
        let builder = ModuleDescription::builder("Fine.\n  @deprecated");
        assert_eq!(
            refusal(builder),
            described("a line in the doc of the module starts with `@`")
        );
    }

    #[test]
    fn a_doc_line_starting_with_at_after_a_tab_is_refused() {
        let builder = ModuleDescription::builder("\t@deprecated");
        assert_eq!(
            refusal(builder),
            described("a line in the doc of the module starts with `@`")
        );
        let after_form_feed = ModuleDescription::builder("\u{b}\u{c} @x");
        assert_eq!(
            refusal(after_form_feed),
            described("a line in the doc of the module starts with `@`")
        );
    }

    #[test]
    fn an_at_sign_that_is_not_first_on_its_line_is_accepted() {
        let built = ModuleDescription::builder("Mail me @ home.\nuser@host").build();
        assert!(built.is_ok());
    }

    #[test]
    fn a_multi_line_parameter_doc_is_refused() {
        let multi = "a\nb";
        let cases: [(ModuleDescriptionBuilder, &str); 4] = [
            (
                empty().function(Function::new("f", "").param("p", STRING, multi)),
                "parameter `p` of `f`",
            ),
            (
                empty().function(Function::new("f", "").variadic(ANY, multi)),
                "the variadic parameter of `f`",
            ),
            (empty().field("x", STRING, multi), "field `x`"),
            (
                empty().record(Record::new("R", "").field("x", STRING, multi)),
                "field `x` of record `R`",
            ),
        ];
        for (builder, owner) in cases {
            assert_eq!(
                refusal(builder),
                described(&format!("the doc of {owner} must be one line"))
            );
        }
    }

    #[test]
    fn a_multi_line_module_function_or_record_doc_is_accepted() {
        let built = ModuleDescription::builder("One.\nTwo.")
            .function(Function::new("f", "One.\nTwo."))
            .record(Record::new("R", "One.\nTwo."))
            .build();
        assert!(built.is_ok());
    }

    #[test]
    fn a_literal_with_a_quote_backslash_or_newline_is_refused() {
        let cases = [
            ("a\"b", "a\\\"b"),
            ("a\\b", "a\\\\b"),
            ("a\nb", "a\\nb"),
            ("a\rb", "a\\rb"),
        ];
        for (text, shown) in cases {
            let builder =
                empty().function(Function::new("f", "").param("p", LuaType::literal(text), ""));
            assert_eq!(
                refusal(builder),
                described(&format!(
                    "string literal type `{shown}` must not contain a double quote, a backslash or a line break"
                ))
            );
        }
    }

    #[test]
    fn a_literal_refusal_escapes_its_text() {
        let message =
            refusal(empty().function(Function::new("f", "").returns(LuaType::literal("a\nb"))));
        assert!(!message.contains('\n'));
        assert!(message.contains("a\\nb"));
    }

    #[test]
    fn an_invalid_name_is_reported_as_an_invalid_lua_name() {
        let builder = empty().function(Function::new("end", ""));
        assert_eq!(
            refusal(builder),
            "invalid Lua name `end`: must not be a Lua reserved word"
        );
    }

    #[test]
    fn every_kind_of_name_goes_through_the_lua_name_rules() {
        let cases = [
            empty().function(Function::new("f", "").param("9p", STRING, "")),
            empty().field("a-b", STRING, ""),
            empty().record(Record::new("", "")),
            empty().record(Record::new("R", "").field("local", STRING, "")),
        ];
        for builder in cases {
            assert!(refusal(builder).starts_with("invalid Lua name `"));
        }
    }

    #[test]
    fn a_signature_problem_in_a_record_field_names_the_field_and_record() {
        let find = callback(Signature::new().param("text", STRING).param("text", STRING));
        let builder = empty().record(Record::new("Compiled", "").field("find", find, ""));
        assert_eq!(
            refusal(builder),
            "invalid module description: parameter `text` of field `find` of record `Compiled` is described twice"
        );
    }

    #[test]
    fn a_signature_problem_in_a_module_field_or_nested_parameter_names_the_member() {
        let handlers = LuaType::array(callback(Signature::new().param("a", ANY).param("a", ANY)));
        assert_eq!(
            refusal(empty().field("on", handlers, "")),
            "invalid module description: parameter `a` of `on` is described twice"
        );

        let two_tails = callback(Signature::new().variadic(ANY).variadic(ANY));
        assert_eq!(
            refusal(empty().function(Function::new("f", "").param("p", two_tails, ""))),
            "invalid module description: `f` has two variadic parameters"
        );
    }

    #[test]
    fn a_function_typed_module_field_is_refused() {
        let handler = callback(Signature::new().param("payload", ANY));
        for ty in [handler.clone(), LuaType::optional(handler)] {
            assert_eq!(
                refusal(empty().field("on", ty, "")),
                described("field `on` has a function type; describe it with `function` instead")
            );
        }
    }

    const NESTED_POSITION: &str = "a function type in parameter `p` of `f` must not hold a function, a union or a parenthesised type as a parameter or return type";

    fn with_callback_param(inner: LuaType) -> ModuleDescriptionBuilder {
        empty().function(Function::new("f", "").param("p", inner, ""))
    }

    #[test]
    fn a_function_type_inside_a_function_type_is_refused() {
        let inner = callback(
            Signature::new()
                .param("cb", callback(Signature::new().returns(STRING)))
                .param("x", INTEGER),
        );
        assert_eq!(
            refusal(with_callback_param(inner)),
            described(NESTED_POSITION)
        );
    }

    #[test]
    fn a_union_or_parenthesised_type_inside_a_function_type_is_refused() {
        let in_return = callback(Signature::new().returns(LuaType::optional(STRING.or(INTEGER))));
        assert_eq!(
            refusal(with_callback_param(in_return)),
            described(NESTED_POSITION)
        );

        let in_param = callback(Signature::new().param("a", STRING.or(INTEGER)));
        assert_eq!(
            refusal(with_callback_param(in_param)),
            described(NESTED_POSITION)
        );

        let in_variadic = callback(Signature::new().variadic(LuaType::array(STRING.or(INTEGER))));
        assert_eq!(
            refusal(with_callback_param(in_variadic)),
            described(NESTED_POSITION)
        );
    }

    #[test]
    fn a_union_inside_a_map_inside_a_function_type_is_accepted() {
        let inner = callback(Signature::new().param("a", LuaType::map(INTEGER.or(STRING), STRING)));
        assert!(with_callback_param(inner).build().is_ok());
    }

    #[test]
    fn a_function_type_inside_a_map_is_refused() {
        let ty = LuaType::map(STRING, callback(Signature::new()));
        assert_eq!(
            refusal(empty().field("x", ty, "")),
            described("a map type in field `x` must not hold a function type")
        );
        let in_key = LuaType::map(LuaType::array(callback(Signature::new())), STRING);
        assert_eq!(
            refusal(empty().field("x", in_key, "")),
            described("a map type in field `x` must not hold a function type")
        );
    }

    #[test]
    fn the_owner_of_an_outer_type_is_named_for_returns_and_record_fields() {
        let in_return =
            Function::new("f", "").returns(LuaType::map(STRING, callback(Signature::new())));
        assert_eq!(
            refusal(empty().function(in_return)),
            described("a map type in return value of `f` must not hold a function type")
        );
        let in_record =
            Record::new("R", "").field("x", LuaType::map(STRING, callback(Signature::new())), "");
        assert_eq!(
            refusal(empty().record(in_record)),
            described("a map type in field `x` of record `R` must not hold a function type")
        );
    }

    #[test]
    fn build_reports_the_first_problem_in_declaration_order() {
        let bad_doc_first = ModuleDescription::builder("a\rb")
            .function(Function::new("f", ""))
            .field("f", STRING, "");
        assert_eq!(
            refusal(bad_doc_first),
            described("the doc of the module must not contain a carriage return")
        );

        let duplicate_before_reference = empty()
            .function(Function::new("f", "").returns(LuaType::record("Nope")))
            .field("f", STRING, "");
        assert_eq!(
            refusal(duplicate_before_reference),
            described("`f` is described twice")
        );
    }

    #[test]
    fn members_are_checked_before_records() {
        let builder = empty()
            .record(Record::new("end", ""))
            .field("x\u{e9}", STRING, "");
        assert_eq!(
            refusal(builder),
            "invalid Lua name `x\u{e9}`: must contain only ASCII letters, digits and underscores"
        );
    }

    #[test]
    fn a_required_parameter_may_follow_an_optional_one() {
        let built = empty()
            .function(
                Function::new("f", "")
                    .optional("a", STRING, "")
                    .param("b", STRING, ""),
            )
            .build();
        assert!(built.is_ok());
    }

    #[test]
    fn an_unused_record_is_accepted() {
        assert!(empty().record(Record::new("Unused", "")).build().is_ok());
    }

    #[test]
    fn a_record_field_keeps_its_type_for_the_reader() {
        let description = empty()
            .record(Record::new("R", "").field("x", STRING, "d"))
            .build()
            .unwrap();
        let record = description.records().first().unwrap();
        let field: &Field = record.fields().first().unwrap();
        assert_eq!(field.ty(), &STRING);
    }
}
