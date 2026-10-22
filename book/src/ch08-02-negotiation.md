# How the Host Decides

Between reading a manifest and running an extension, the host makes its decision in three steps.
Each step can only narrow what the extension gets, never widen it.

```
  manifest ──► ① ceiling ──► ② approver ──► ③ start
               │              │              │
               what the       a final yes    build an engine with the
               host will      or no          agreed policy, then run
               ever allow                    the entry script
```

## ① The ceiling

The **ceiling** is the most the host is ever willing to allow, written as an ordinary policy. The
manifest's request is compared against it, item by item, and each item ends up in one of three
states:

| Result | Means |
|---|---|
| granted | it was asked for, and it is inside the ceiling |
| reduced | it was asked for, and only part of it is inside the ceiling |
| denied | it was asked for, and it is outside the ceiling |

A denied *required* capability stops the extension right here. Nothing else runs.

Limits work the same way. If the manifest asks for 8 MB of memory and the ceiling allows 64 MiB, the
extension gets 8 MB. If it asks for more than the ceiling allows, it gets the ceiling.

## ② The approver

If the request survives the ceiling, an **approver** gives the final answer. `airsl` comes with two:

| Approver | Answer |
|---|---|
| `ManifestApprover` | yes to whatever survived the ceiling |
| `DenyAll` | no, always |

A host can write its own. A common choice is to approve extensions from a trusted registry
automatically, and to ask a person about anything else. The approver can only say no to what the
ceiling allowed. It can never add to it.

## ③ Start

Only now does `airsl` build an engine, under the agreed policy, and run the extension's entry script.
By the time any extension code runs, both checks have passed. That order is enforced by the types in
the Rust API, not by convention: there is no way to start an extension that has not been approved.

## Seeing the decision

`airsl ext doctor` shows the whole decision without running any of the extension's code:

```console
$ airsl ext doctor ./word-count
extension:    word-count 0.1.0 (api 1, entry main.lua)
events:       none
ceiling:
  language:     restricted
  grants:       none
  memory:       67108864 bytes
  instructions: 100000000 instructions
negotiated:
  language:     restricted
  grants:       none
  memory:       8388608 bytes
  instructions: 1000000 instructions
requested:
  module       regex                    granted
decision:     approve
```

`ceiling` is what the host offered, here the `confined` defaults. `negotiated` is what the extension
will actually run under: the same grants, and the smaller limits its manifest asked for.

When a request is outside the ceiling, the report says which item and why:

```console
$ airsl ext doctor ./greedy
...
requested:
  fs.read      /                        denied    (outside the granted read roots: none)
decision:     deny — fs.read `/`: outside the granted read roots: none
```

To widen the ceiling, pass the same grant flags `airsl run` takes. `--var NAME=VALUE` supplies a
manifest variable.

## Firing an event

`airsl ext fire` loads the extension for real and fires one event at it. The payload is read from
standard input, and the result is printed as JSON:

```console
$ echo '{"text":"the quick brown fox"}' | airsl ext fire ./word-count count
{"longest":"quick","words":4}
```

It goes through exactly the same steps a Rust host would, so it is a quick way to try an extension
before you write the host.
