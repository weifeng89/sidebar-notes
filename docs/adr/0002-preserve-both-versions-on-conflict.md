# Preserve both versions when an external edit overlaps a draft

When a watched file changes while the sidebar holds unsaved edits, keep both the disk version and a conflict copy of the draft, then ask the user to reconcile them. The original spec proposed letting the disk version win, but that would discard the in-app draft; silent overwrite is unacceptable for a personal knowledge library.
