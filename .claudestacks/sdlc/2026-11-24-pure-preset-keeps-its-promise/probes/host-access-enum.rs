// The enum declaration exactly as spec §3.1 shows it, compiled as a library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum HostAccess { Stdin, Stdout, Stderr, Clock, Cwd, Require, Zones }
