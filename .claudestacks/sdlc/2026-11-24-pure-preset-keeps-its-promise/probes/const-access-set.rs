// Throwaway probe: a private bit set over HostAccess can be filled inside `const fn`, so
// `GrantSet::declared()` and `Policy::confined()` can stay `const`, unlike a BTreeSet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum HostAccess { Stdin, Stdout, Stderr, Clock, Cwd, Require, Zones }

impl HostAccess {
    pub const ALL: [Self; 7] =
        [Self::Stdin, Self::Stdout, Self::Stderr, Self::Clock, Self::Cwd, Self::Require, Self::Zones];
    const fn bit(self) -> u8 { 1 << self as u8 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct AccessSet(u8);

impl AccessSet {
    const fn none() -> Self { Self(0) }
    const fn all() -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < HostAccess::ALL.len() { bits |= HostAccess::ALL[i].bit(); i += 1; }
        Self(bits)
    }
    const fn with(self, a: HostAccess) -> Self { Self(self.0 | a.bit()) }
    const fn contains(self, a: HostAccess) -> bool { self.0 & a.bit() != 0 }
}

pub struct GrantSet { access: AccessSet }
impl GrantSet {
    pub const fn declared() -> Self { Self { access: AccessSet::all() } }
    pub const fn sealed() -> Self { Self { access: AccessSet::none() } }
    pub const fn with_access(self, a: HostAccess) -> Self { Self { access: self.access.with(a) } }
    pub const fn allows(&self, a: HostAccess) -> bool { self.access.contains(a) }
}

const DECLARED: GrantSet = GrantSet::declared();
const STDERR_ONLY: GrantSet = GrantSet::sealed().with_access(HostAccess::Stderr);

fn main() {
    println!("declared allows every member: {}", HostAccess::ALL.iter().all(|a| DECLARED.allows(*a)));
    println!("sealed+stderr allows stderr: {}, stdout: {}", STDERR_ONLY.allows(HostAccess::Stderr), STDERR_ONLY.allows(HostAccess::Stdout));
    println!("bits declared={:#09b}", DECLARED.access.0);
}
