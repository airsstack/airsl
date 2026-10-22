# Embedding airsl in Rust

So far you have used `airsl` as a command. This chapter uses it as a library, from inside your own
Rust program.

That is the case `airsl` was really built for. Imagine a program that lets its users customise it
with scripts: a build tool with user-written rules, a server with user-written handlers, an editor
with user-written commands. Your program runs code you did not write, so it needs to decide what that
code may reach. Everything you have seen from the command line, every grant and every limit, is
available from Rust, because the `airsl` command is itself a small program built on this library.

Add it to your project:

```console
$ cargo add airsl
```

Building it needs a C compiler, because `airsl` compiles Lua from its C source and links it into
your program. There is no system Lua to install, and nothing to configure.
