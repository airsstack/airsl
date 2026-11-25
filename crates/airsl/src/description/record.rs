//! A named table shape a signature can name as a parameter, return or field type, and the fields
//! it carries.
//!
//! Its own module because a record is a different concept from a function: it names a shape rather
//! than a call, and `LuaType::record` refers to it by name from any signature in the module.
//!
//! Responsibilities:
//!
//! - [`Record`] and [`Field`], and the builder method that appends a field.
//! - Keeping fields in the order they were added.
//!
//! Non-responsibilities: validating record or field names, or checking that two fields do not
//! clash. They are stored as given; [`ModuleDescriptionBuilder::build`]
//! checks them.
//!
//! [`ModuleDescriptionBuilder::build`]: super::ModuleDescriptionBuilder::build

use super::LuaType;

/// One named, typed, documented field of a [`Record`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    name: String,
    ty: LuaType,
    doc: String,
}

impl Field {
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

/// A named table shape whose fields keep their declaration order.
///
/// A function names a record in its signature with [`LuaType::record`].
///
/// # Examples
///
/// ```
/// use airsl::description::{LuaType, Record};
///
/// let output = Record::new("Output", "What a process printed.")
///     .field("stdout", LuaType::STRING, "Captured standard output.")
///     .field("status", LuaType::INTEGER, "Exit status.");
/// # let _ = output;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    name: String,
    doc: String,
    fields: Vec<Field>,
}

impl Record {
    /// A record called `name` with no fields yet.
    #[must_use]
    pub fn new(name: &str, doc: &str) -> Self {
        Self {
            name: name.to_owned(),
            doc: doc.to_owned(),
            fields: Vec::new(),
        }
    }

    /// Appends a field after the ones already added.
    #[must_use]
    pub fn field(mut self, name: &str, ty: LuaType, doc: &str) -> Self {
        self.fields.push(Field {
            name: name.to_owned(),
            ty,
            doc: doc.to_owned(),
        });
        self
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn doc(&self) -> &str {
        &self.doc
    }

    pub(crate) fn fields(&self) -> &[Field] {
        &self.fields
    }
}

#[cfg(test)]
mod tests {
    use super::{Field, Record};
    use crate::description::LuaType;

    #[test]
    fn fields_keep_declaration_order() {
        let record = Record::new("Stat", "File metadata.")
            .field("size", LuaType::INTEGER, "Length in bytes.")
            .field("kind", LuaType::STRING, "What the path is.")
            .field("readonly", LuaType::BOOLEAN, "Whether writes are refused.");
        let names: Vec<&str> = record.fields().iter().map(Field::name).collect();
        assert_eq!(names, ["size", "kind", "readonly"]);
        let types: Vec<&LuaType> = record.fields().iter().map(Field::ty).collect();
        assert_eq!(
            types,
            [&LuaType::INTEGER, &LuaType::STRING, &LuaType::BOOLEAN]
        );
        let docs: Vec<&str> = record.fields().iter().map(Field::doc).collect();
        assert_eq!(
            docs,
            [
                "Length in bytes.",
                "What the path is.",
                "Whether writes are refused."
            ]
        );
    }

    #[test]
    fn a_record_carries_its_name_and_doc() {
        let record = Record::new("Output", "What a process printed.");
        assert_eq!(record.name(), "Output");
        assert_eq!(record.doc(), "What a process printed.");
        assert!(record.fields().is_empty());
    }
}
