# AGENTS.md — Max's AI Assistant Principles

> **My personal, stack-agnostic ruleset for AI coding assistants** (Claude Code,
> Codex, Copilot, Cursor, …). This is the distilled common core I reuse across
> every project — React Native / Expo, Next.js, Tauri, Phaser, and beyond.
>
> Drop this file in a repo root as `AGENTS.md` (point `CLAUDE.md` /
> `.github/copilot-instructions.md` at it), then add a short **project-specific**
> section for the stack, design system, and commands. Edit the real file, not the
> pointer.

These rules are **fundamental — do not violate them.** When a project's own
instructions conflict, the project wins; otherwise these apply everywhere.

---

## Philosophy — keep it simple

These override cleverness. When in doubt, choose the simpler path.

- **Keep it simple.** Always prefer simple, readable solutions over complex ones.
- **Light & readable code.** Don't overcomplicate implementations.
- **No unnecessary abstractions.** Write straightforward code that's easy to
  understand — abstract only when it genuinely pays off. Don't add
  managers/factories/event-buses where a direct call does the job.
- **No костыли (no hacks).** Never ship hacky quick fixes that are hard to
  support later.
- **Durable solutions.** Prefer straightforward designs with clear data flow and
  low coupling.
- **Avoid:** unnecessary condition chains, duplicate logic, hidden side effects.
- **Scope honesty.** If a proper fix needs a larger scope, explain the tradeoff
  and **ask me** instead of shipping a fragile patch.

---

## ⚠️ Read the docs first

Frameworks change. The APIs, conventions, and file structure in the current
version often differ from older versions **and from your training data.**

- **Before writing any framework code, read the relevant guide for the exact
  version this project uses** — versioned online docs or the docs shipped in
  `node_modules` / the SDK. Heed deprecation notices.
- **Don't guess APIs.** If a type or signature is unfamiliar, open the docs
  rather than inventing one that "looks right."
- The project-specific section names the framework, version, and doc URL.

---

## Performance is the product

A premium, native feel comes from speed. Treat every perceptible delay, dropped
frame, or jank as a **bug**, not as optional polish.

- **Instant interaction.** Every click/keystroke gives feedback within one frame.
  Never block the UI/main thread on I/O.
- **Optimistic UI.** Update in-memory state immediately, persist in the
  background, reconcile on success / roll back + toast on failure. The user never
  waits on the network or filesystem.
- **Debounce expensive work** (autosave, search) — but keep in-memory state live
  so the UI stays instant.
- **Virtualize long lists.** Never render thousands of DOM nodes; never `.map()`
  a large dataset straight into the view. Use a virtualizer / `FlashList` /
  equivalent.
- **Hot paths are sacred.** In per-frame / per-render loops: no heavy
  allocations, no creating objects in the loop, no synchronous layout reads.
  Cache references; reuse via pools where relevant.
- **Lean bundle.** Per-icon imports, lazy-load heavy/rare views, no giant
  dependency for a small job.
- **Optimize images** (proper component, caching, correct sizes).
- **Memoize only where profiling shows a real win** — modern React + the React
  Compiler reduce the need for manual `useMemo`/`useCallback`; don't add them
  reflexively.

---

## UI/UX principles (fundamental — do not violate)

1. **Premium & fresh.** Generous spacing, soft rounded corners, subtle elevation,
   clear hierarchy. No cramped or default-looking UI.
   - **No tiny fonts or cramped controls — ever.** Small text/buttons read as a
     *cheap* interface and are uncomfortable to use. Bias every size up. Body
     text floor ~15–16px, never below ~13px even for captions. Controls get
     comfortable height and real padding; primary actions and inputs are tall
     with large touch targets. When unsure, go bigger.
   - **Never leave a single word alone on the last line** of a text block (an
     orphan/widow) — it looks cheap. Keep it on one line, balance the wrap, or
     shorten the copy.
2. **Responsive to its target.** Mobile-first for phones, desktop-first within a
   resizable window for desktop apps. Fluid grids; never fixed-pixel layouts that
   break across devices or window sizes. Respect safe-area insets on mobile.
3. **Friendly & approachable.** Clear labels, human copy. **Always design the
   empty, loading, and error states — not just the happy path.**
4. **Fast & optimized.** See **Performance is the product** — it is the spec.
5. **No anti-patterns.** No prop-drilling marathons, no god-components, no inline
   magic numbers/colors, no `any`, no unhandled promises, no fetching in loops,
   no duplicated state, no blocking the UI thread, no layout shift.
6. **Accessible.** Semantic elements, ARIA where needed (`aria-label`, `role`),
   visible focus-visible rings, full keyboard operability, sufficient contrast,
   alt text, correct heading order. Decorative icons are hidden from screen
   readers (`aria-hidden`).

---

## Design system discipline

- **Match the reference mockups.** If the project ships designs, the
  implementation matches them — layout, palette, spacing, accent usage.
- **Tokens, never hardcoded values.** Define colors and shared design tokens as
  CSS variables / theme tokens (e.g. Tailwind `@theme`). **Never hardcode hex in
  components.** Support both light and dark mode as first-class where relevant;
  follow the OS theme by default with a manual override.
- **Restraint = premium.** Accent colors are purposeful ("look here" states:
  active nav, primary action, key figure). Everything else is neutral. No
  rainbow.

### Sizing — the 4-pt grid

**Every dimension is a multiple of 4.** Spacing, padding, margin, gap, width,
height, border-radius, icon sizes, offsets — all snap to the 4-pt grid (4, 8, 12,
16, 20, 24, 32, 40, 48, 64…). This keeps rhythm consistent and the UI premium.

- Prefer **spacing/radius tokens** (or stock framework scale classes) over raw
  numbers. Reach for a raw number only when no token fits — and it **must** be
  divisible by 4.
- Applies to fixed `width`/`height` too (images, avatars, sidebars): `140`,
  `160`, `240`, `256` — never `170`, `185`, `33`.
- **Font sizes and line heights are the only exception** — they follow the
  typographic scale, not the 4-pt grid.
- **No arbitrary bracket values** in utility-class frameworks — never
  `text-[15px]`, `w-[170px]`. Use the nearest stock scale class. Don't bend the
  type scale in `@theme` to force a px size.

---

## Code & file structure rules

- **Hard limit: 100–150 lines per file.** If a file grows past this, split —
  extract subcomponents, hooks, helpers, types, configs. Prefer many small
  focused files over few large ones. (Applies to every language, backend
  included.)
- **One component/class per file — strictly.** Each goes in its own PascalCase
  file matching its name. Do not put a second component or a helper alongside
  another component, even when both are small. Compose via imports.

### Group by entity/feature — the threshold-4 rule

This is the rule that gets violated most. **Read it before creating every new
file.** A flat dump of a dozen files in one directory is a bug, not a "refactor
later."

1. **Threshold = 4.** As soon as a directory would hold **4 files of the same
   level** (not counting `index.ts`) — **STOP.** Before adding that 4th file,
   group related ones into a subfolder.
2. **Group by entity/feature, not by type.** Files about one thing live together
   (`player/` → `Player.ts`, `player-movement.ts`, `player-combat.ts`), not
   scattered across `ai/`, `config/`, `gameobjects/`.
3. **3+ files about one entity → its own folder.** No exceptions.
4. **A folder with 2+ files gets an `index.ts`** (barrel) for clean imports from
   outside.
5. **Don't over-fragment.** A single standalone file does NOT get its own folder.
   A folder appears when a **cluster forms (≥3 related files)**, not preemptively.

### Extract — don't inline complexity

- **Complex logic inside a loop → its own named function.**
- **Conditionally-rendered components → extracted**, not nested inline.
- **List items → their own component**, not inline JSX/markup in `.map()`.

```tsx
// ❌ Don't
{items.map((i) => <div className={i.active ? '…' : '…'}>{i.name}</div>)}

// ✅ Do
{items.map((item) => <ListItem key={item.id} item={item} />)}
```

### Path aliases

Set up `@/*` → `src/*` (in `tsconfig.json` paths **and** the bundler's alias
config — both are required). Use `@/…` imports, never deep `../../..` chains.

### Constants & magic numbers

**No magic numbers** in logic. Speeds, timings, damage, limits, breakpoints —
named constants (`ALL_CAPS`), grouped in a `config/`/`constants` location. Route
paths, enum-like sets, and other shared literals come from a **single `as const`
source**, referenced everywhere so they can't drift.

---

## TypeScript & state

- **TypeScript everywhere**, strict (`strict: true`, ideally `noUnusedLocals` /
  `noUnusedParameters`). **No `any`.** Never weaken strict to move faster — narrow
  with an explicit, commented cast instead of silencing the check.
- **Type every async/IO boundary** (API calls, `invoke`, fetch). Define the
  return type and a typed args object; centralize these wrappers so components
  never call them stringly-typed.
- **State:** server/async state → a dedicated server-state layer (e.g. TanStack
  Query) as the single source of truth; global UI/app state → a store (Zustand);
  local ephemeral state → `useState`/`useReducer`. **Never mirror server data
  into client state** or keep two copies in sync by hand.
- Keep components **presentational**; push data fetching into hooks / server
  components.

### TypeScript conventions

- ✅ **Interfaces** prefixed with `I` (`IButton`, `INote`).
- ✅ **Type aliases** prefixed with `T` (`TOrderStatus`, `TDirection`).
- ✅ Explicit `type` imports: `import type { FC } from 'react'`.
- ✅ **Union types** for variants: `'primary' | 'outline' | 'ghost'`.
- ✅ **Enums via `as const` objects**, not TS `enum`:

```ts
export const OrderStatusEnum = {
  Pending: 'pending',
  InProgress: 'in_progress',
  Delivered: 'delivered'
} as const

export type OrderStatusEnum =
  (typeof OrderStatusEnum)[keyof typeof OrderStatusEnum]
```

---

## Naming conventions

- ✅ **PascalCase** — components, classes, and types.
- ✅ **camelCase** — variables, functions, methods, props.
- ✅ **ALL_CAPS** — constants and computed config values.
- ✅ **Descriptive function names** (`formatPrice`, `transformUserToState`,
  `spawnNailHitbox`).
- ✅ **Action-based handler names** (`onPressOrder`, `onChangeSearch`,
  `onSelectNote`).
- ✅ **Boolean values get an `is` / `has` / `should` / `can` prefix**:
  `isLoading`, `isOpen`, `hasError`, `canSubmit`, `shouldRender`. Applies to
  state, props, variables, and derived values.
- ✅ **No cryptic one-letter arguments.** Name every parameter for what it is:
  `(note) =>` not `(n) =>`, `(state) => state.id` not `(s) => s.id`,
  `(time, delta) =>` not `(t, d) =>`. The only tolerated short name is the
  throwaway `_`.
- ✅ **Component/entity names are plain obvious nouns:** `Badge`, `Card`,
  `Button`, `Sidebar`, `Editor`, `List`, `Row`, `Modal`, `Panel`, `Player`,
  `Enemy`.
- ❌ **No vague layout words** as names or prefixes: no `Shell`, `Hero*`,
  `Widget`.

### File naming

- ✅ **Component/class files → PascalCase** (`RestaurantCard.tsx`, `Player.ts`).
- ✅ **Everything else → kebab-case** (`use-auth.ts`, `format-price.ts`,
  `order-schema.ts`).
- ✅ **Framework routing files keep their required names** (`_layout.tsx`,
  `page.tsx`, `+not-found.tsx`, `route.ts`, …).
- ✅ **Backend follows its language idiom** (Rust → `snake_case` files,
  functions, modules; commands named for the action: `read_note`, `write_note`).

---

## Exports, classes, icons

- ✅ **Named exports** for all components, utilities, functions, classes,
  constants.
- ✅ **`export default` ONLY for framework route files** that require it
  (`page.tsx`, `_layout.tsx`, …). Everywhere else, named exports.
- ✅ **Barrel exports** (`index.ts`) for clean imports — but don't barrel so
  aggressively that it hurts tree-shaking of icons / heavy deps.
- ✅ **Class names composed with a `cn()` helper** (`clsx` + `tailwind-merge`).
  Use `cn(...)` for every conditional/variant class — never hand-build class
  strings with template literals or concatenation:

```ts
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
```

- ✅ **Icons from a per-icon library import** (tree-shaking) — never hand-roll
  inline `<svg>` for standard icons. For long, data-driven icon lists use a local
  sprite, not a bundled icon set. Icon sizes snap to the 4-pt grid.

---

## Animation

- Use the lightest tool that works: **CSS transitions / Web Animations** for
  simple feedback (hover, focus, press); reach for an animation library only when
  orchestration / gestures / springs genuinely need it.
- ✅ **GPU-accelerated only:** animate `transform` and `opacity`. **Avoid**
  layout-triggering (`width`, `height`, `top`, `left`) and paint-triggering
  (`color`, `background`, `box-shadow`) props in hot paths.
- ✅ Keep durations **short (120–300ms)** and purposeful; use easing; avoid janky
  continuous loops. Use `will-change` sparingly and remove it after.
- ✅ **Respect `prefers-reduced-motion`** — disable/replace motion accordingly.

---

## Safety — don't mutate external state on your own

- **Don't run DB migrations or raw SQL yourself.** When schema or queries are
  needed, write the SQL into a clearly labelled block / `.sql` file and hand it to
  me to execute. Generate types only after I confirm it ran.
- **Don't widen permissions/capabilities "just to make it work."** Scope every
  new capability to the narrowest path; explain the scope.
- **Don't launch the dev server / app unless I explicitly ask** — I run it
  myself.
- Treat anything outward-facing or hard to reverse as **confirm-first.**

---

## Definition of done (check before finishing)

- [ ] Read the relevant **versioned framework docs** before writing code.
- [ ] Solution is the **simplest** that works — no костыли, no needless
      abstraction.
- [ ] **Feels instant:** no UI/main-thread blocking; optimistic updates;
      debounced heavy work; long lists virtualized; no per-frame allocations.
- [ ] Every touched file is **≤ 150 lines**; large ones were split (frontend
      *and* backend).
- [ ] **One component/class per file**; correct folder; **threshold-4 grouping**
      respected (no ≥4 flat same-level files; clusters in named folders with
      `index.ts`); `@/*` aliases, no `../../..`.
- [ ] **Named exports** (default only for framework route files); `cn()` for
      classes; per-icon imports; no inline `<svg>` for standard icons.
- [ ] **TS conventions** followed (`I`/`T` prefixes, `as const` enums, `type`
      imports, **no `any`**, strict not weakened); every async/IO boundary typed.
- [ ] **Colors/spacing from tokens**, not hardcoded; all dimensions on the 4-pt
      grid; no arbitrary bracket values.
- [ ] **Matches the reference mockups**; light and dark mode both correct where
      relevant.
- [ ] **Loading / empty / error states** handled on every data path.
- [ ] **Animations** use `transform`/`opacity` only and respect reduced motion.
- [ ] **No external state mutated on my behalf** — SQL handed over, permissions
      scoped, dev server not launched.
- [ ] **Lint and build pass.**

---

## Project-specific: vrox.vpn

Desktop VPN client on **Tauri v2** (Rust backend + React 19 frontend,
no Next.js/React Native — most of the React-Native-flavored rules above
don't apply literally here: no Zustand/TanStack Query, no Tailwind
tokens/4-pt grid, no FlashList. Keep the *philosophy* — simple,
readable, no needless abstraction — drop the React-Native-specific
tooling rules).

**Stack:**
- `app/` — Tauri app. Rust backend (`app/src-tauri/src/`), plain React +
  CSS frontend (`app/src/App.tsx` — intentionally a single file, not yet
  split; CSS in `App.css`, no CSS framework).
- `macos-ext/` — Swift `NEPacketTunnelProvider` extension (Xcode
  project, generated via `xcodegen` from `project.yml` — the
  `.xcodeproj` itself is gitignored, regenerate with `xcodegen generate`
  if missing, then re-apply manual signing overrides in
  `project.pbxproj` documented in `macos-ext/build-testflight.sh`).
- `packaging/hysteria2-patch/` — Go: forked `apernet/hysteria` +
  `netunnel` package (gomobile-bound for the macOS extension).

**Platform split is real, not cosmetic:** `engine/linux.rs` and
`engine/macos.rs` implement the same public API
(`spawn_client`/`kill_client`/`enable_killswitch`/...) with completely
different mechanisms (pkexec+nftables vs NetworkExtension) — `engine.rs`
re-exports whichever matches `target_os` via `#[cfg]`. Don't unify them
into shared abstractions just for symmetry; the platforms are genuinely
different here.

**Doc-comments carry history, not just behavior.** This codebase has
repeatedly hit subtle platform bugs (entitlement conflicts, signature
quirks, race conditions) that took real debugging to find. When you fix
one, write the *why* (what failed, how it was diagnosed) in a comment
at the fix site, in Russian, matching the existing style — not just
*what* the code does. Future debugging sessions rely on this.

**Commands:**
- Linux build: `cd app && pnpm tauri build`
- macOS local build (DMG, not for distribution): `./macos-ext/build-release.sh`
- macOS TestFlight build: `./macos-ext/build-testflight.sh`
- Typecheck frontend: `cd app && npx tsc --noEmit -p .`
- Rust compile check: `cd app/src-tauri && cargo build --no-default-features`

**Distribution/updates:** Linux ships `.deb`, self-updates via
`version.json` in this repo + a privileged helper script. macOS ships
via **TestFlight only** (no App Store, no custom updater) — see
`docs/ARCHITECTURE.md`.
