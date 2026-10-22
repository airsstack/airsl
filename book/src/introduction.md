# Introduction

Every project grows a small crowd of scripts. A hook that runs before each commit. A tool that
rebuilds an index. A helper an editor calls every time you save. They start as a few lines of shell,
then someone needs JSON and reaches for Python, then someone needs a package and reaches for Node.

Each of those scripts can do anything the person running it can do. It can read your SSH keys,
delete your home directory, or call out to the network. Usually none of them do, but nothing stops
them, and nothing tells you what each one actually touches.

`airsl` is a different arrangement. It runs Lua scripts inside a Rust program, and the program
decides what each script may reach. A script that wants to read a file has to be *given* a folder it
may read. A script that wants to run `git` has to be *given* `git`. Everything else is refused, and
the refusal tells you exactly what was missing.

## Two ways to use it

You can use `airsl` in two ways, and this book covers both.

```
  As a command                        As a library
  ────────────                        ────────────
  $ airsl run script.lua              let engine = Engine::builder()
                                          .policy(Policy::confined())
  You write Lua. The command line         .build()?;
  says what the script may touch.
                                      You write Rust. Your program decides
                                      what the scripts it runs may touch.
```

The command is the quickest way in, and it is how most of the early chapters work. The library is
for when you are building a program that runs other people's scripts: plugins, extensions, user
configuration. The same rules apply in both, because the command is built on the library.

## Who this book is for

You are an engineer. You have written scripts before, and you may have written Rust. You do not need
to know Lua. Lua is small, and this book explains what it uses as it goes.

If you only want to run scripts, chapters 1 to 5 are enough. If you want to embed `airsl` in your own
program, read on into chapters 6 to 8. Chapter 9 is for the curious: how it is built, and why.

## How this book is organised

| Chapter | What you will do |
|---|---|
| [1. Getting Started](ch01-00-getting-started.md) | Install `airsl` and run your first script |
| [2. A First Project](ch02-00-first-project.md) | Build a small tool, hit the sandbox, and test it |
| [3. Asking Permission](ch03-00-asking-permission.md) | Grant files, environment variables and programs |
| [4. When Scripts Misbehave](ch04-00-when-scripts-misbehave.md) | Stop runaway scripts and fail safely |
| [5. The Toolbox](ch05-00-the-toolbox.md) | Tour the modules every script can use |
| [6. Embedding airsl in Rust](ch06-00-embedding.md) | Run scripts from your own program |
| [7. Writing Your Own Module](ch07-00-your-own-module.md) | Give scripts a new capability |
| [8. Extensions](ch08-00-extensions.md) | Load third-party code with a negotiated set of permissions |
| [9. Under the Hood](ch09-00-under-the-hood.md) | How `airsl` is built, and what it can and cannot promise |

Every chapter has its own small example, so you can start wherever you like. The
[appendix](appendix-00.md) collects the reference material: every command-line flag, every module
function, and the three presets side by side.
