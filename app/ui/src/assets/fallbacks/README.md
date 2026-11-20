# TEMPORARY placeholder art

Everything in this directory is a generated stand-in (see `scripts/gen-placeholders.mjs`).
It exists only so the UI has an image for every entity during development.
**It must be replaced by real, original, licensed illustrated art (PNG/WebP at 1x/2x) before release.**

File naming contract used by `src/lib/fallbackImage.ts`:
`placeholder-profile-0..15`, `placeholder-mod-0..2`, `placeholder-server-0`,
`placeholder-account-steve`, `placeholder-hero`. Swap the files (or the glob in `fallbackImage.ts`); nothing else changes.
