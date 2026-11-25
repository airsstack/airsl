//! The parameters, variadic tail and returns of a function, and the unnamed function type a
//! callback argument has.
//!
//! Its own module because [`Function`] and [`Signature`] share one parameter list: a `Function` is
//! a `Signature` plus the names and docs a rendered file needs, while a `Signature` stands alone
//! as the type of a handler. It sits beside `LuaType` because each holds the other.
//!
//! Responsibilities:
//!
//! - [`Presence`], [`Param`], [`Signature`] and [`Function`], and their builder methods.
//! - The `fun(...)` spelling of a signature.
//!
//! Non-responsibilities: validating parameter names or function names. They are stored as given;
//! [`ModuleDescriptionBuilder::build`] checks them.
//!
//! [`ModuleDescriptionBuilder::build`]: super::ModuleDescriptionBuilder::build

use super::LuaType;

/// Whether a call must supply a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The call must supply it.
    Required,
    /// The call may leave it out, spelled `name?`.
    Optional,
}

/// One named parameter of a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    name: String,
    ty: LuaType,
    presence: Presence,
    doc: String,
    /// Whether `variadic` had already been called when this parameter was added.
    after_variadic: bool,
}

impl Param {
    fn new(name: &str, ty: LuaType, presence: Presence, doc: &str) -> Self {
        Self {
            name: name.to_owned(),
            ty,
            presence,
            doc: doc.to_owned(),
            after_variadic: false,
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn ty(&self) -> &LuaType {
        &self.ty
    }

    pub(crate) const fn presence(&self) -> Presence {
        self.presence
    }

    pub(crate) fn doc(&self) -> &str {
        &self.doc
    }

    pub(crate) const fn follows_variadic(&self) -> bool {
        self.after_variadic
    }
}

/// A function type: its parameters, an optional variadic tail and its returns.
///
/// A signature carries no documentation. It spells only as a `fun(...)` type, such as the
/// handler a module accepts as an argument.
///
/// # Examples
///
/// ```
/// use airsl::description::{LuaType, Signature};
///
/// let handler = Signature::new().param("payload", LuaType::ANY).returns(LuaType::ANY);
/// # let _ = LuaType::function(handler);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Signature {
    params: Vec<Param>,
    /// Boxed because a `LuaType` can hold a `Signature`, so the two would otherwise have no finite size.
    variadic: Option<Box<LuaType>>,
    /// How many times `variadic` was called. The tail holds only the last type, so a second call
    /// would otherwise leave no trace for [`ModuleDescriptionBuilder::build`] to refuse.
    ///
    /// [`ModuleDescriptionBuilder::build`]: super::ModuleDescriptionBuilder::build
    variadic_calls: usize,
    returns: Vec<LuaType>,
}

impl Signature {
    /// An empty signature: no parameters, no variadic tail, no returns.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            params: Vec::new(),
            variadic: None,
            variadic_calls: 0,
            returns: Vec::new(),
        }
    }

    /// Appends a parameter, remembering whether it was added after the variadic tail.
    fn push_param(&mut self, mut param: Param) {
        param.after_variadic = self.variadic.is_some();
        self.params.push(param);
    }

    /// Sets the variadic tail and counts the call.
    fn set_variadic(&mut self, ty: LuaType) {
        self.variadic = Some(Box::new(ty));
        self.variadic_calls += 1;
    }

    /// Appends a required parameter.
    #[must_use]
    pub fn param(mut self, name: &str, ty: LuaType) -> Self {
        self.push_param(Param::new(name, ty, Presence::Required, ""));
        self
    }

    /// Appends an optional parameter.
    #[must_use]
    pub fn optional(mut self, name: &str, ty: LuaType) -> Self {
        self.push_param(Param::new(name, ty, Presence::Optional, ""));
        self
    }

    /// Sets the variadic tail, spelled `...: ty`.
    #[must_use]
    pub fn variadic(mut self, ty: LuaType) -> Self {
        self.set_variadic(ty);
        self
    }

    /// Appends a return type. Call it again for a function that returns several values.
    #[must_use]
    pub fn returns(mut self, ty: LuaType) -> Self {
        self.returns.push(ty);
        self
    }

    pub(crate) fn params(&self) -> &[Param] {
        &self.params
    }

    pub(crate) fn variadic_type(&self) -> Option<&LuaType> {
        self.variadic.as_deref()
    }

    pub(crate) const fn variadic_calls(&self) -> usize {
        self.variadic_calls
    }

    pub(crate) fn returns_types(&self) -> &[LuaType] {
        &self.returns
    }

    /// The signature as a `fun(...)` type; `prefix` is `<root>.<module>`, the qualifier a record
    /// name takes.
    pub(crate) fn spell(&self, prefix: &str) -> String {
        let mut parts: Vec<String> = self
            .params
            .iter()
            .map(|param| {
                let mark = match param.presence {
                    Presence::Required => "",
                    Presence::Optional => "?",
                };
                format!("{}{mark}: {}", param.name, param.ty.spell(prefix))
            })
            .collect();
        if let Some(tail) = &self.variadic {
            parts.push(format!("...: {}", tail.spell(prefix)));
        }
        let mut text = format!("fun({})", parts.join(", "));
        if !self.returns.is_empty() {
            let returns: Vec<String> = self.returns.iter().map(|ty| ty.spell(prefix)).collect();
            text.push_str(": ");
            text.push_str(&returns.join(", "));
        }
        text
    }
}

/// A named, documented function a module exposes.
///
/// # Examples
///
/// ```
/// use airsl::description::{Function, LuaType};
///
/// let hash_file = Function::new("hash_file", "Hashes a file's bytes.")
///     .param("path", LuaType::STRING, "File to hash.")
///     .optional("algorithm", LuaType::STRING, "Defaults to sha256.")
///     .returns(LuaType::STRING);
/// # let _ = hash_file;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    name: String,
    doc: String,
    signature: Signature,
    variadic_doc: String,
}

impl Function {
    /// A function called `name` with no parameters and no returns yet.
    #[must_use]
    pub fn new(name: &str, doc: &str) -> Self {
        Self {
            name: name.to_owned(),
            doc: doc.to_owned(),
            signature: Signature::new(),
            variadic_doc: String::new(),
        }
    }

    /// Appends a required parameter.
    #[must_use]
    pub fn param(mut self, name: &str, ty: LuaType, doc: &str) -> Self {
        self.signature
            .push_param(Param::new(name, ty, Presence::Required, doc));
        self
    }

    /// Appends an optional parameter.
    #[must_use]
    pub fn optional(mut self, name: &str, ty: LuaType, doc: &str) -> Self {
        self.signature
            .push_param(Param::new(name, ty, Presence::Optional, doc));
        self
    }

    /// Sets the variadic tail, spelled `...: ty`.
    #[must_use]
    pub fn variadic(mut self, ty: LuaType, doc: &str) -> Self {
        self.signature.set_variadic(ty);
        doc.clone_into(&mut self.variadic_doc);
        self
    }

    /// Appends a return type. Call it again for a function that returns several values.
    #[must_use]
    pub fn returns(mut self, ty: LuaType) -> Self {
        self.signature.returns.push(ty);
        self
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn doc(&self) -> &str {
        &self.doc
    }

    pub(crate) const fn signature(&self) -> &Signature {
        &self.signature
    }

    pub(crate) fn variadic_doc(&self) -> &str {
        &self.variadic_doc
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "tests assert on values they just built")]

    use super::{Function, Param, Presence, Signature};
    use crate::description::LuaType;

    #[test]
    fn param_optional_and_variadic_record_their_presence() {
        let signature = Signature::new()
            .param("a", LuaType::STRING)
            .optional("b", LuaType::INTEGER)
            .variadic(LuaType::NUMBER);
        let presences: Vec<Presence> = signature.params().iter().map(Param::presence).collect();
        assert_eq!(presences, [Presence::Required, Presence::Optional]);
        assert_eq!(signature.params().len(), 2);
        assert_eq!(signature.variadic_type(), Some(&LuaType::NUMBER));
    }

    #[test]
    fn returns_accumulate_in_order() {
        let signature = Signature::new()
            .returns(LuaType::STRING)
            .returns(LuaType::INTEGER)
            .returns(LuaType::BOOLEAN);
        assert_eq!(
            signature.returns_types(),
            [LuaType::STRING, LuaType::INTEGER, LuaType::BOOLEAN]
        );
    }

    #[test]
    fn a_signature_keeps_parameters_in_declaration_order() {
        let signature = Signature::new()
            .param("c", LuaType::STRING)
            .optional("a", LuaType::STRING)
            .param("b", LuaType::STRING);
        let names: Vec<&str> = signature.params().iter().map(Param::name).collect();
        assert_eq!(names, ["c", "a", "b"]);
    }

    #[test]
    fn a_function_keeps_its_docs_beside_its_parameters() {
        let function = Function::new("hash_file", "Hashes a file.")
            .param("path", LuaType::STRING, "File to hash.")
            .optional("algorithm", LuaType::STRING, "Defaults to sha256.")
            .variadic(LuaType::ANY, "Ignored extras.")
            .returns(LuaType::STRING);
        assert_eq!(function.name(), "hash_file");
        assert_eq!(function.doc(), "Hashes a file.");
        let docs: Vec<&str> = function
            .signature()
            .params()
            .iter()
            .map(Param::doc)
            .collect();
        assert_eq!(docs, ["File to hash.", "Defaults to sha256."]);
        assert_eq!(function.variadic_doc(), "Ignored extras.");
        assert_eq!(function.signature().variadic_type(), Some(&LuaType::ANY));
        assert_eq!(function.signature().returns_types(), [LuaType::STRING]);
    }

    #[test]
    fn a_parameter_added_after_the_variadic_is_marked() {
        let signature = Signature::new()
            .param("a", LuaType::STRING)
            .variadic(LuaType::ANY)
            .param("b", LuaType::STRING)
            .optional("c", LuaType::STRING);
        let marks: Vec<bool> = signature
            .params()
            .iter()
            .map(Param::follows_variadic)
            .collect();
        assert_eq!(marks, [false, true, true]);

        let function =
            Function::new("f", "")
                .variadic(LuaType::ANY, "")
                .param("late", LuaType::STRING, "");
        assert!(
            function
                .signature()
                .params()
                .first()
                .unwrap()
                .follows_variadic()
        );
    }

    #[test]
    fn every_variadic_call_is_counted() {
        assert_eq!(Signature::new().variadic_calls(), 0);
        let twice = Signature::new()
            .variadic(LuaType::ANY)
            .variadic(LuaType::STRING);
        assert_eq!(twice.variadic_calls(), 2);
        let function = Function::new("f", "")
            .variadic(LuaType::ANY, "")
            .variadic(LuaType::ANY, "");
        assert_eq!(function.signature().variadic_calls(), 2);
    }

    #[test]
    fn a_signature_parameter_carries_no_doc() {
        let signature = Signature::new().param("x", LuaType::ANY);
        assert_eq!(signature.params().first().unwrap().doc(), "");
    }
}
