# Issue tracker: local Markdown

Sidebar Notes development issues and specs live under `.scratch/<feature-slug>/` in this repository. They are local working files, not GitHub issues.

- Spec: `.scratch/<feature-slug>/spec.md`
- Tickets: `.scratch/<feature-slug>/issues/NN-<slug>.md`, one ticket per file
- Triage: a `Status:` line near the top, using `docs/agents/triage-labels.md`
- Discussion: append under `## Comments`

When a skill asks to publish or fetch a ticket, write or read the corresponding Markdown file here. Public, durable architecture decisions belong in `docs/adr/`, not in tickets.
