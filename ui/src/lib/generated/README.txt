Generated TypeScript bindings for the Rust IPC contract. Do not edit by hand.

Source of truth: the Rust types in crates/app (ts-rs derives) and the command
table in crates/app/src/dispatch.rs. Generator: crates/app/src/bindings.rs.

Regenerate after changing any IPC type, command or event:

    cargo test -p mango-app export_ts_bindings -- --ignored

`cargo test -p mango-app committed_bindings_are_current` (part of
`cargo test --workspace`) regenerates into a scratch directory and fails when
these files differ, so drift is caught in CI.

Files
  <Type>.ts    one file per type that crosses the boundary (args, results, events, AppError)
  index.ts     barrel: `import type { ProfileView, CommandMap } from './generated'`
  commands.ts  CommandMap: command name -> { args, result }
  events.ts    EventMap: event name -> payload

Conventions: JSON is camelCase; u64/i64 are `number` (all values stay below 2^53);
`Option<T>` is `T | null`; an optional argument is `field?: T | null`; fields
the server omits when empty (skip_serializing_if) are `field?: T`.
