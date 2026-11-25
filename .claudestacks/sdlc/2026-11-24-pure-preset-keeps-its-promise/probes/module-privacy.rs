// Throwaway probe: can a crate-root sibling (like `require_loader`) reach an item in a private
// child module of `modules` (like `modules::access`)? First as `mod access;`, then `pub(crate)`.
mod modules {
    #[cfg(private)]
    mod access { pub(crate) fn detail() -> &'static str { "detail" } }
    #[cfg(not(private))]
    pub(crate) mod access { pub(crate) fn detail() -> &'static str { "detail" } }
}
mod require_loader {
    pub(crate) fn refuse() -> &'static str { crate::modules::access::detail() }
}
fn main() { println!("{}", require_loader::refuse()); }
