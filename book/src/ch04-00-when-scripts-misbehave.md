# When Scripts Misbehave

Grants stop a script from touching what it should not. They do nothing about a script that simply
goes wrong: one that loops forever, eats all your memory, or has a typo in it.

This chapter covers the two tools `airsl` has for that:

- **limits**, which stop a script that runs too long or uses too much memory
- **failing open**, which keeps a broken script from breaking whatever ran it
