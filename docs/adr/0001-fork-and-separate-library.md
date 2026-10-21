# Keep Stik intact and add a separate Sidebar Notes library

Sidebar Notes starts from a full Stik fork and preserves the existing capture, search, settings, and other functions. The sidebar owns a separate user-visible library rooted in real directories and files. This avoids a risky module deletion pass and prevents the new Prompt/Notes/Docs structure from changing the meaning of existing Stik notes. A later migration can be explicit and reversible.
