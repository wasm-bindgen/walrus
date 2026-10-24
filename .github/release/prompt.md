You are preparing the walrus {{version}} release, the release after {{prev}}.
Your working directory is the repository at `main` with the versions already
bumped. You can read any file, but you can only edit `CHANGELOG.md`.
Maintainers review everything you change in the release PR.

The `## Unreleased` section of `CHANGELOG.md` collects the entries since
{{prev}}. Turn it into the {{version}} section:

- Insert `## {{version}}` with `Released {{date}}.` below it, so that the
  current entries move under it, as in previous releases. Leave
  `## Unreleased` above it with its `Released YYYY-MM-DD.` line and every
  `###` section holding only `* TODO (or remove section if none)`.
- In the {{version}} section, keep the `###` sections that have entries, in
  the order Added, Changed, Deprecated, Removed, Fixed, Security, and drop the
  rest.
- Fix clear mistakes only: typos, broken links, duplicates and entries under
  the wrong section. Don't otherwise reword entries.
- Compare the entries against the merged PRs below. Add an entry for each
  user-facing PR that has none, in the style of the surrounding ones. Leave
  out dependency bumps, CI changes, bot configuration and other internal
  changes unless they matter to users, and list any PR you were unsure about
  in `notes`.
- walrus is a 0.x library, so a patch release must not contain breaking
  changes to public types and functions. If the entries or PRs suggest one,
  say so in `notes`.

In `notes`, write short Markdown for the maintainers reviewing the release
PR: PRs you left out or were unsure about, and anything else that needs a
human decision. Leave out what you changed, since the diff shows that.
