---
status: done
created: 2026-11-25
---

# Milestone: require the root table

## Goal

`require("<root>")` returns the root table under every preset; scripts using the global keep
working; the root name wins over a same-named user file.

## Non-goals

- Removing the global.
