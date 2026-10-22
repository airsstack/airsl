# Installation

`airsl` ships as a single prebuilt binary for macOS, Linux and Windows. You do not need Rust, Lua or
a C compiler to run it.

## macOS and Linux

```console
$ curl --proto '=https' --tlsv1.2 -LsSf https://github.com/airsstack/airsl/releases/latest/download/airsl-cli-installer.sh | sh
```

## Windows

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/airsstack/airsl/releases/latest/download/airsl-cli-installer.ps1 | iex"
```

The installer puts `airsl` in `~/.local/bin` (`%USERPROFILE%\.local\bin` on Windows) and adds that
folder to your `PATH`. Open a new terminal so the change takes effect, then check it worked:

```console
$ airsl --version
airsl 0.1.4
```

If you would rather the installer left your `PATH` alone, set `AIRSL_CLI_NO_MODIFY_PATH=1` before
running it.

## Building from source

If you already have a Rust toolchain, you can build `airsl` yourself:

```console
$ cargo install --locked airsl-cli
```

This needs a C compiler, because `airsl` compiles Lua itself from its C source and links it into the
binary. That is also why the prebuilt binary needs nothing else installed: Lua is already inside it.

## Other ways to install

Each [release](https://github.com/airsstack/airsl/releases) also carries a plain archive for every
platform, a checksum for each archive, and a signed record of which build produced it. The
[installation appendix](appendix-04-installation.md) covers downloading by hand, choosing between the
Linux builds, and verifying what you downloaded.
