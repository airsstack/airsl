# Extensions

So far, the person running a script has also decided what it may touch: you wrote the grant flags,
or your program built the policy. An **extension** turns that around. It is code someone else wrote,
which comes with its own description of what it needs. Your program reads that description, decides
how much of it to allow, and only then runs the code.

This is the model Redis uses for its scripts, and the model editors use for plugins. The extension
asks; the host decides.

```
  extension.toml says          your program allows          the extension gets
  ───────────────────          ───────────────────          ──────────────────
  read  ~/notes          ∩     read  ~/notes, ~/docs   =    read  ~/notes
  run   git, tar               run   git                    ✗ refused: tar is not allowed
```

In this chapter you will:

1. write a small extension that counts words
2. describe what it needs in a manifest
3. see how a host bounds what an extension may ask for
4. try it from the command line, then load it from Rust

## Two kinds of extension

| Kind | How it runs | Example |
|---|---|---|
| a **script** | runs once from top to bottom, then ends | everything in chapters 1 to 5 |
| a **registered extension** | loads once, registers handlers, and is called each time an event happens | this chapter |

A registered extension pays the cost of starting once, and is then called again and again. As
[chapter 6](ch06-04-reuse.md) showed, calling into an engine that already exists is many times
faster than building a new one, so this shape is what makes extensions cheap enough to call on every
event.
