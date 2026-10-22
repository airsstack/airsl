# D. Installation Details

## Archives

Each [release](https://github.com/airsstack/airsl/releases) has one archive per platform. You can
download one by hand instead of running the installer.

| Archive | Runs on | Needs at runtime |
|---|---|---|
| `aarch64-apple-darwin` | macOS on Apple Silicon | nothing beyond macOS |
| `x86_64-apple-darwin` | macOS on Intel | nothing beyond macOS |
| `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | Linux with glibc | a glibc at least as new as the build machine's |
| `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` | any Linux, including Alpine | nothing; fully static |
| `x86_64-pc-windows-msvc` | Windows | nothing beyond Windows |

On Linux, the installer picks the glibc build when your system's glibc is new enough, and the musl
build otherwise. Both run the same scripts in the same way.

## Verifying a download

Every archive has a `.sha256` file next to it. The installer checks the archive against it when
`sha256sum` is available.

Every archive also carries a GitHub artifact attestation, a signed record that ties the file to this
repository and the build that made it. That catches even an archive swapped together with its
checksum file:

```console
$ gh attestation verify airsl-cli-x86_64-unknown-linux-musl.tar.xz --repo airsstack/airsl
```

Before any release is published, every archive is unpacked on the platform it targets and made to
run the example scripts, pass the example tests, and refuse what it was not granted. An archive that
fails is not published.

## Unsigned binaries

The binaries are not signed with an Apple or Windows code-signing certificate. That only matters for
an archive downloaded through a web browser. On macOS, Gatekeeper refuses to run it until you clear
the download mark with `xattr -d com.apple.quarantine airsl`. On Windows, SmartScreen warns the first
time. The installers download archives themselves, so neither happens.

## A shadowed `airsl`

If you installed `airsl` with `cargo install` before, `~/.cargo/bin/airsl` may come first on your
`PATH`. The installer warns when that happens. `cargo uninstall airsl-cli` removes the old copy.

## The latest `main`

To build the newest unreleased code:

```console
$ cargo install --git https://github.com/airsstack/airsl --locked airsl-cli
```
