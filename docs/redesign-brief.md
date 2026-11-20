# MangoClient redesign: Claude Design prompt

Paste everything below the line into Claude Design (claude.ai, Design). Attach `src/assets/icon.png` and a couple of screenshots of the current launcher if you can.

---

Design the desktop UI for **MangoClient**, a Minecraft launcher for Windows, macOS and Linux. Produce a canvas of artboards at 1280x800 (dark theme first, light theme for the Home and Settings screens as a check), plus one component sheet.

## Who it is for
Players aged 14 to 35 who run several modded setups (PvP, survival modpacks, shaders) and switch between them daily. They want to hit Play fast, and also manage mods, accounts and servers without friction. Power users should feel respected; new users should not feel lost.

## Personality
Confident, a little playful, craft-focused. Think Linear's precision, Arc's warmth, Modrinth App's clarity, a game's sense of place. It must look made by a person with taste, not generated. The brand is a mango: warm, juicy, slightly cheeky. Pixel/blocky Minecraft heritage is a texture, not a costume.

## Image-first rule (hard requirement)
Never use placeholder letters or initials. Every profile, mod, modpack, server and account is always represented by an image. Where no real image exists, use an illustrated fallback tile (pixel or isometric Minecraft blocks, in the spirit of Modrinth App's block tiles, but with our own art style): profiles get a block/biome tile picked deterministically from their id; mods get a block-with-jar tile per type; servers get a server-block tile; accounts always show the skin head. The final build uses real illustrated image files for these, not code-drawn shapes; in the mockup, make them look like finished illustrations so the real art can drop in. Design the fallback set (at least 16 profile tiles plus the mod, shader, pack and server tiles) on the component sheet, and use them in every screen.

## Explicit anti-slop rules
- No purple-to-blue gradients, no glowing neon blobs, no glassmorphism stacks, no cards inside cards inside cards.
- No emoji as icons. Do not hand-draw icons: use the Phosphor Icons set (bold weight for navigation and primary actions, regular elsewhere, one weight per context), Lucide only for gaps. Character comes from layout, type, color and imagery, not custom glyphs.
- No identical grids of rounded rectangles for everything. Vary the rhythm: a big hero, a dense table, a rail, a timeline.
- No stock "rocket / sparkle / magic wand" illustrations, no marketing filler copy ("Supercharge your experience"). Write short, plain, specific microcopy.
- No centered-everything. Use strong left alignment, a real baseline grid, and asymmetric layouts.
- Real content in every screen: actual mod names (Sodium, Lithium, Iris, Fabric API, Xaero's Minimap), versions (1.21.4, 1.20.6), playtimes, server names, believable numbers. No "Lorem ipsum", no "Item 1".

## Visual direction
- **Palette**: near-black warm ink base (`#0b0c0e` to `#1c1f24`), warm off-white text (`#f4f1ea`). Primary accent **mango `#ffad32`** used sparingly and decisively (Play, active state, key numbers). Secondary: leaf green `#86b86b` for ok, a deep tomato `#ed665c` for errors. Propose one extra supporting hue for content types (mods / packs / shaders), muted, never neon.
- **Imagery**: use per-profile cover art and Minecraft-world imagery (panoramas, biome vistas, block close-ups at integer-scaled nearest-neighbour). Hero areas should show the world, with a scrim for legibility. Show how an empty profile gets a generated cover from a biome palette instead of a flat colour.
- **Type**: pair a characterful display face with a neutral UI face (suggest real options available on Google Fonts, for example a squared grotesk for headings and Inter or Geist for UI). Tabular numerals for versions, RAM, ping, playtime. Minimum 13px body.
- **Shape**: a deliberate radius system (for example 4 / 8 / 14), hairline borders, shadows only where something floats. One memorable signature shape or detail (for example a notched corner, or a stepped pixel edge on primary buttons).
- **Motion** (annotate on the artboards): Play button states, the launch progress sequence, view transitions under 250ms, a satisfying "installed" confirmation. Respect reduced motion.

## Information architecture (fix the current confusion)
Current problems: "profiles" appear in the rail, a Profiles page, a Start dropdown and tabs; Accounts is in three places; Discover is not tied to a profile; the console is hidden in a drawer.
New model:
- **Left rail**: Home, Library (all profiles), Discover, Servers, and a pinned list of recent profiles. Account switcher and Settings at the bottom. Collapsible to icons.
- **Home**: last-played profile as a big hero with Play, profile switcher, version/loader/Java/RAM chips, playtime, and quick-join servers. A compact "continue" strip of recent profiles. Not a mod list.
- **Profile page**: hero + tabs: Overview, Content (mods, packs, shaders in one list with type filter), Worlds & Screenshots, Logs, Settings. Discover opened from inside a profile is scoped to it, with a clear "Installing to: <profile>" bar.
- **Live game panel**: when a game runs, a persistent, readable console/log panel and a Stop control, not a cryptic pill.

## Screens to design (all with real states)
1. **Home**, idle, with Play ready.
2. **Home, launching**: staged progress (Preparing, Java, Loader, Downloading, Account, Launching) with a believable 62% download, then **Running** state with live console panel open.
3. **Library**: profile grid/list toggle, sort, search, "New profile".
4. **Profile page, Content tab**: 38 mods with enabled toggles, update-available badges, dependency owners ("required by Iris"), type filter, drag-and-drop zone for `.jar` and `.mrpack`.
5. **Discover**: Modrinth search for "shaders", filters (type, loader, version compatibility), result list with icons, downloads, install button states (Install, Installing 40%, Installed, Update), and the scoped "Installing to" bar. A mod detail drawer.
6. **New profile** flow: a 3-step stepper (Name & look, Version & loader, Resources) replacing the overloaded modal. Version picker with search, release/snapshot filter, loader recommendation.
7. **Accounts**: two Microsoft accounts and one offline, with a large 3D skin viewport (a posed Minecraft player, soft floor shadow), skin change, expired-session state.
8. **Servers**: live status cards (online, MOTD, ping, players) with Quick Join, star/partner servers first.
9. **Settings**: Game, Performance (RAM slider showing system memory), Java, Launcher, Storage (usage bar by category), Updates. Clear section nav.
10. **Onboarding**: 5 steps, memorable first-run, language picker (English / Deutsch), sign-in, RAM.
11. **Empty, error and loading states**: no accounts, no mods, offline, failed install (with a specific cause and action), skeleton loaders. Each with a purposeful, non-clip-art treatment.
12. **Update available** and **Updating** takeover screen.

## Window chrome
Custom title bar that adapts per platform: native-feeling traffic lights on macOS, standard caption buttons on Windows and Linux. Design the draggable region, breadcrumb, and a subtle update indicator.

## Component sheet
Buttons (primary Play in all states, secondary, ghost, destructive), toggle, checkbox, segmented control, select with search, slider, tabs, chips/badges, tooltip, toast, modal and drawer, list row (dense), card (cover), progress (determinate, indeterminate, staged), keyboard focus ring, form field with error. Show the spacing and type scale, the radius system, and the Phosphor icons in use (at least 40, named).

## Constraints
- Must work at 960x600 minimum; show how the rail collapses.
- Keyboard-complete, 4.5:1 contrast for text, visible focus.
- Every string must be translatable: show German text lengths (about 30% longer) in at least two screens.
- Must be implementable in a webview with CSS and one WebGL canvas for the skin.

## Deliverable
Artboards for the screens above, the component sheet, a tokens table (colour, type, spacing, radius, shadow, motion), and short annotations for interactions. State your design rationale in five sentences at the top of the canvas.
