# Subproject 5-E Stage 1: Slash Command System

> **For agentic workers:** Plan was approved via ExitPlanMode. Implement task-by-task, one commit per task. Write tool calls stay under ~150 lines — use Edit to append when a file grows beyond that.

**Goal:** Add slash-command infrastructure to the lemurclaw-gui Composer — generic popup, command catalog, dispatch architecture — plus ~16 core commands. This is the `<SlashPopup>` row from spec §4.2 that subproject 3 deferred.

**Stage 1 scope:** Infrastructure + core command subset. Stage 2 (remaining ~30 commands) and Stage 3 (`<MentionPopup>` + `<FileSearchPopup>`) land in later PRs.

**Relationship to 5-C:** Stage 1's `/diff` command returns "coming in 5-C". 5-C (the actual `<DiffViewerModal>`) lands after 5-E.

**Why its own subproject:** Spec §4.2 originally placed `<SlashPopup>` in subproject 3 alongside Composer, but subproject 3 shipped only the bare textarea. 50+ commands × no wire metadata (verified: `available_commands` doesn't exist on the wire despite spec's claim) × several inapplicable to web GUI → needs its own plan.

## Architecture (three layers)

```
Composer (textarea)
  ├─ onChange → detect leading "/" → open <ComposerPopup>
  ├─ <ComposerPopup>          ← generic popover, absolute-positioned
  │     renders filtered items; ↑↓/Enter/Esc handled by Composer
  │     onChoose(item) → dispatch
  └─ submit() → if text is /cmd form, dispatch; else send turn
```

**Layers:**
1. **`<ComposerPopup>`** — generic text-anchored popover. Slash/mention/file-search share it (Stage 1 only uses slash).
2. **`slashCommands.ts`** — client-side hardcoded command catalog. Each command: `{ name, description, category, available?, dispatch }`.
3. **`slashCommandTypes.ts`** — `SlashCommandResult` discriminated union. Composer/App routes by `kind`:
   - `{ kind: 'sendTurn'; input: unknown[] }` — send turn/start (e.g. /init)
   - `{ kind: 'openSettings'; surface: SettingsSurface }` — open SettingsModal tab (e.g. /skills)
   - `{ kind: 'openModal'; modal: ModalKind }` — open existing modal (e.g. /model /theme)
   - `{ kind: 'localAction'; action: LocalAction }` — local op (e.g. /clear /new /quit)
   - `{ kind: 'notImplemented'; message: string }` — intercept + message (e.g. /diff until 5-C)

## File Structure

**New files:**
- `assets/src/components/ComposerPopup.tsx` — generic popover shell
- `assets/src/components/__tests__/ComposerPopup.test.tsx`
- `assets/src/components/composer/slashCommands.ts` — command catalog
- `assets/src/components/composer/slashCommandTypes.ts` — result union + category enums
- `assets/src/components/composer/dispatch.ts` — dispatch router
- `assets/src/components/composer/__tests__/slashCommands.test.ts`
- `assets/src/components/composer/__tests__/dispatch.test.ts`

**Modified files:**
- `assets/src/components/Composer.tsx` — popup state + "/" detection + submit routing
- `assets/src/components/__tests__/Composer.test.tsx` — slash integration tests
- `assets/src/app/App.tsx` — `onSlashCommand` handler + LocalAction
- `assets/src/app/__tests__/App.test.tsx` — slash command integration tests
- `assets/src/components/settings/SettingsModal.tsx` — accept `initialSurface` prop
- `assets/src/styles.css` — `.composer-popup` etc.

## Commit Plan

One commit per task:
1. Task 1 — types + catalog skeleton
2. Task 2 — `<ComposerPopup>` component
3. Task 3 — Composer wires popup + slash detection
4. Task 4 — dispatch router + per-command tests
5. Task 5 — App.tsx wiring + LocalAction
6. Task 6 — styles + final regression

Expected total ~500-700 lines (under 800-line PR ceiling). All frontend, no Rust changes.

---

## Task 1: `SlashCommandResult` types + catalog skeleton

**Files:**
- Create: `assets/src/components/composer/slashCommandTypes.ts`
- Create: `assets/src/components/composer/slashCommands.ts`
- Create: `assets/src/components/composer/__tests__/slashCommands.test.ts`

### `slashCommandTypes.ts`

Defines the discriminated union + context + command interface. Imports `SettingsSurface` from the existing SettingsModal so openSettings commands stay type-safe against renames.

```ts
import type { SettingsSurface } from '../settings/SettingsModal';

export type SlashCommandCategory =
  | 'sendTurn'
  | 'openSettings'
  | 'openModal'
  | 'localAction'
  | 'notImplemented';

export type LocalAction = 'clear' | 'new' | 'quit';

/** Existing top-level modals. Kept in sync with App.tsx's ModalKind union. */
export type ModalKind = 'model' | 'theme' | 'transcript' | 'settings';

export type SlashCommandResult =
  | { kind: 'sendTurn'; input: unknown[] }
  | { kind: 'openSettings'; surface: SettingsSurface }
  | { kind: 'openModal'; modal: ModalKind }
  | { kind: 'localAction'; action: LocalAction }
  | { kind: 'notImplemented'; message: string };

export interface SlashCommandContext {
  threadId: string | null;
  openSettings: (surface: SettingsSurface) => void;
  openModal: (modal: ModalKind) => void;
  localAction: (action: LocalAction) => void;
}

export interface SlashCommand {
  /** WITHOUT leading "/". Lowercase, no spaces. */
  name: string;
  description: string;
  category: SlashCommandCategory;
  available?: (ctx: SlashCommandContext) => boolean;
  dispatch: (args: string, ctx: SlashCommandContext) => SlashCommandResult;
}
```

### `slashCommands.ts` (16 commands)

Each command carries its own `dispatch`. Stage 1 implements all of them; the catalog is the single source of truth for both the popup and the dispatch router.

```ts
import type { SlashCommand } from './slashCommandTypes';

const INIT_PROMPT =
  'Create an AGENTS.md file in the repository root with instructions for working in this codebase.';

export const SLASH_COMMANDS: SlashCommand[] = [
  // sendTurn
  { name: 'init', description: 'Create an AGENTS.md file with instructions for Codex', category: 'sendTurn',
    dispatch: () => ({ kind: 'sendTurn', input: [{ type: 'text', text: INIT_PROMPT, text_elements: [] }] }) },
  { name: 'review', description: 'Review my current changes and find issues', category: 'sendTurn',
    dispatch: (args) => ({ kind: 'sendTurn', input: [{ type: 'text', text: args ? `Review this: ${args}` : 'Review my current changes and find issues', text_elements: [] }] }) },
  { name: 'compact', description: 'Summarize conversation to prevent hitting the context limit', category: 'sendTurn',
    dispatch: () => ({ kind: 'sendTurn', input: [{ type: 'text', text: 'Compact the conversation', text_elements: [] }] }) },
  // openModal
  { name: 'model', description: 'Choose what model and reasoning effort to use', category: 'openModal',
    dispatch: (_a, ctx) => { ctx.openModal('model'); return { kind: 'openModal', modal: 'model' }; } },
  { name: 'theme', description: 'Choose a syntax highlighting theme', category: 'openModal',
    dispatch: (_a, ctx) => { ctx.openModal('theme'); return { kind: 'openModal', modal: 'theme' }; } },
  // openSettings (8 commands)
  { name: 'permissions', description: 'Choose what Codex is allowed to do', category: 'openSettings',
    dispatch: (_a, ctx) => { ctx.openSettings('permissions'); return { kind: 'openSettings', surface: 'permissions' }; } },
  { name: 'memories', description: 'Configure memory use and generation', category: 'openSettings',
    dispatch: (_a, ctx) => { ctx.openSettings('memories'); return { kind: 'openSettings', surface: 'memories' }; } },
  { name: 'skills', description: 'Use skills to improve how Codex performs specific tasks', category: 'openSettings',
    dispatch: (_a, ctx) => { ctx.openSettings('skills'); return { kind: 'openSettings', surface: 'skills' }; } },
  { name: 'hooks', description: 'View and manage lifecycle hooks', category: 'openSettings',
    dispatch: (_a, ctx) => { ctx.openSettings('hooks'); return { kind: 'openSettings', surface: 'hooks' }; } },
  { name: 'mcp', description: 'List configured MCP tools', category: 'openSettings',
    dispatch: (_a, ctx) => { ctx.openSettings('mcp'); return { kind: 'openSettings', surface: 'mcp' }; } },
  { name: 'apps', description: 'Manage apps', category: 'openSettings',
    dispatch: (_a, ctx) => { ctx.openSettings('apps'); return { kind: 'openSettings', surface: 'apps' }; } },
  { name: 'plugins', description: 'Browse plugins', category: 'openSettings',
    dispatch: (_a, ctx) => { ctx.openSettings('plugins'); return { kind: 'openSettings', surface: 'plugins' }; } },
  { name: 'experimental', description: 'Toggle experimental features', category: 'openSettings',
    dispatch: (_a, ctx) => { ctx.openSettings('experimental'); return { kind: 'openSettings', surface: 'experimental' }; } },
  // localAction
  { name: 'clear', description: 'Clear the terminal and start a new chat', category: 'localAction',
    dispatch: (_a, ctx) => { ctx.localAction('clear'); return { kind: 'localAction', action: 'clear' }; } },
  { name: 'new', description: 'Start a new chat during a conversation', category: 'localAction',
    dispatch: (_a, ctx) => { ctx.localAction('new'); return { kind: 'localAction', action: 'new' }; } },
  // notImplemented
  { name: 'diff', description: 'Show git diff (including untracked files)', category: 'notImplemented',
    dispatch: () => ({ kind: 'notImplemented', message: 'diff viewer coming in subproject 5-C' }) },
];
```

### `slashCommands.test.ts` (5 sanity tests)

Asserts catalog length, every command has name+description+dispatch, names unique, names lowercase-kebab, categories valid. No dispatch-behavior tests here (those live in Task 4's dispatch.test.ts).

### Verify

```bash
cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/composer/__tests__/slashCommands.test.ts
npx tsc --noEmit
```
Expected: 5/5 pass; tsc clean.

### Commit

```
feat(gui): slash command catalog skeleton (16 commands)

Stage 1 of subproject 5-E. Client-side hardcoded catalog covering
sendTurn (/init /review /compact), openModal (/model /theme),
openSettings (/permissions /memories /skills /hooks /mcp /apps
/plugins /experimental), localAction (/clear /new), and one
notImplemented stub (/diff, awaiting 5-C). Each command carries its
own dispatch fn. Wire protocol unchanged (verified available_commands
doesn't exist on the app-server wire).
```

---

## Task 2: Generic `<ComposerPopup>` component

**Files:**
- Create: `assets/src/components/ComposerPopup.tsx`
- Create: `assets/src/components/__tests__/ComposerPopup.test.tsx`

Generic popover: absolute-positioned, pure presentation + click handling. Keyboard handling lives in the parent (Composer's textarea onKeyDown).

### Props

```ts
interface Props<T> {
  filteredItems: T[];              // already-filtered; popup does no filtering
  renderItem: (item: T, isActive: boolean) => ReactNode;
  activeIndex: number;             // -1 = none
  onChoose: (item: T) => void;
  open: boolean;
  testId?: string;
  emptyText?: string;              // default '(no matches)'
}
```

### Behavior

- `open=false` → return null
- Empty `filteredItems` → render `.composer-popup-empty` with emptyText
- Each item wrapped in `<div role="option" aria-selected={i === activeIndex}>` with `.composer-popup-item` (+`-active` when i === activeIndex)
- Click row → `onChoose(item)`

### Tests (7 cases)

1. `open=false` renders nothing
2. `open=true` renders all filteredItems
3. activeIndex row has `composer-popup-item-active` class
4. Click row fires `onChoose` with that item
5. Empty filteredItems renders emptyText
6. `renderItem` receives correct isActive flag
7. listbox + option aria roles present

### Verify

```bash
npx vitest run src/components/__tests__/ComposerPopup.test.tsx && npx tsc --noEmit
```

### Commit

```
feat(gui): generic <ComposerPopup> popover

Text-anchored popover for the Composer. Stage 1 uses it for slash
commands; Stage 3 will reuse for mention + file-search popups. Pure
presentation + click handling — keyboard lives in the parent.

7 unit tests pass; tsc clean.
```

---

## Task 3: Wire popup into Composer + slash text detection

**Files:**
- Modify: `assets/src/components/Composer.tsx`
- Modify: `assets/src/components/__tests__/Composer.test.tsx`

### Detection logic

```ts
/** Returns the slash token being typed at the start of text, or null.
 *  "/mod" → "mod", "/model foo" → "model", "hello" → null, "/" → "" (trigger). */
function slashToken(text: string): string | null {
  if (!text.startsWith('/')) return null;
  const m = text.match(/^\/(\S*)/);
  return m ? m[1] : '';
}
```

Popup is open iff `slashToken(text) !== null`. `filtered` = `SLASH_COMMANDS.filter(c => token === '' || c.name.startsWith(token))`.

### New prop on Composer

```ts
onSlashCommand: (cmd: SlashCommand, args: string) => void;
```

### New state

- `popupOpen: boolean` (derived from slashToken, but kept explicit so onKeyDown can rely on it)
- `activeIndex: number` (resets to 0 when filter changes)

### `choose(cmd)` helper

- Strip leading `/cmd` from text → args
- Call `onSlashCommand(cmd, args)`
- Clear text + close popup

### `onKeyDown` branches (only when popup open + filtered non-empty)

- `ArrowDown` → `activeIndex = (i + 1) % filtered.length` (wrap)
- `ArrowUp` → `activeIndex = (i - 1 + len) % len` (wrap)
- `Tab` or `Enter` (no shift) → `choose(filtered[activeIndex] ?? filtered[0])`; preventDefault
- `Escape` → close popup; preventDefault (don't send)

When popup closed: original Enter-to-send behavior unchanged.

### `submit()` change

Before sending as turn, check if `token` exactly matches a command name → if so, dispatch via `choose()` instead. This handles the case where user types `/init` and presses Enter without using popup navigation.

### Placeholder update

Change placeholder to mention `/` for commands:
```
'type a message…  (Enter to send, Shift+Enter for newline, / for commands)'
```

### Tests (8 cases)

1. Typing "/" opens popup with all 16 commands
2. Typing "/mo" filters to /model
3. Typing "hello" doesn't open popup
4. ArrowDown/ArrowUp moves active (assert aria-selected on correct row)
5. Enter on popup picks active command, fires onSlashCommand
6. Escape closes popup, doesn't send
7. Newline + "/foo" on second line doesn't open popup (only leading "/" triggers)
8. Typing "/nonexistent" + Enter doesn't call onSlashCommand and doesn't send turn

### Verify

```bash
npx vitest run src/components/__tests__/Composer.test.tsx && npx tsc --noEmit
```

### Commit

```
feat(gui): slash popup + text detection in Composer

Composer now opens <ComposerPopup> when the user types a leading "/",
filters SLASH_COMMANDS by prefix, handles ↑↓/Tab/Enter/Esc, and routes
the chosen command via the new onSlashCommand prop. Non-slash text
behavior unchanged.

8 new Composer tests pass; existing tests unbroken.
```

---

## Task 4: dispatch router + per-command tests

**Files:**
- Create: `assets/src/components/composer/dispatch.ts`
- Create: `assets/src/components/composer/__tests__/dispatch.test.ts`

### `dispatch.ts`

Thin pass-through today; the named entry point leaves room for cross-cutting concerns (logging, metrics) later without changing call sites.

```ts
import type { SlashCommand, SlashCommandContext, SlashCommandResult } from './slashCommandTypes';

export function dispatchSlashCommand(
  cmd: SlashCommand,
  args: string,
  ctx: SlashCommandContext,
): SlashCommandResult {
  return cmd.dispatch(args, ctx);
}
```

### `dispatch.test.ts` — per-command behavior

Uses a `makeCtx()` helper that provides `vi.fn()` for `openSettings`/`openModal`/`localAction`. Looks up commands by name from `SLASH_COMMANDS`. Test cases:

- `/init` → sendTurn with AGENTS.md prompt
- `/review` no args → sendTurn with default review text
- `/review` with args → sendTurn with "Review this: <args>"
- `/compact` → sendTurn with compact prompt
- `/model` → calls `ctx.openModal('model')` + returns `{kind:'openModal', modal:'model'}`
- `/theme` → calls `ctx.openModal('theme')` + matching return
- `/permissions` `/memories` `/skills` `/hooks` `/mcp` `/apps` `/plugins` `/experimental` — `it.each` over these 8: each calls `ctx.openSettings(name)` + returns matching result
- `/clear` `/new` — `it.each`: each calls `ctx.localAction(name)` + returns matching result
- `/diff` → returns `notImplemented` with message matching `/5-C/`

Total: ~14 assertions across `it.each` groups.

### Verify

```bash
npx vitest run src/components/composer/__tests__/dispatch.test.ts && npx tsc --noEmit
```

### Commit

```
feat(gui): slash command dispatch router + per-command tests

Thin dispatchSlashCommand entry point over the catalog. Tests assert
each of the 16 commands produces the correct SlashCommandResult and
fires the expected ctx callback. Covers all 5 categories.
```

---

## Task 5: App.tsx wiring + LocalAction handler

**Files:**
- Modify: `assets/src/app/App.tsx`
- Modify: `assets/src/app/__tests__/App.test.tsx`
- Modify: `assets/src/components/settings/SettingsModal.tsx` (accept `initialSurface` prop)

### SettingsModal.tsx change (one-line API addition)

```tsx
interface Props {
  onClose: () => void;
  initialSurface?: SettingsSurface;  // NEW
}
export function SettingsModal({ onClose, initialSurface = 'permissions' }: Props) {
  const [surface, setSurface] = useState<SettingsSurface>(initialSurface);
  // ...
}
```

### App.tsx changes

**New state:**
- `settingsSurface: SettingsSurface` (default `'permissions'`) — controls which tab SettingsModal opens on
- `clearKey: number` (default `0`) — bumped on `/clear` to force-remount Scrollback

**`handleLocalAction(action)`:**
- `'clear'` → `setClearKey(k => k + 1)` (force-remount Scrollback via `key={clearKey}` on its wrapper div). This is UI-only clear — server-side conversation state unchanged. NOT equivalent to TUI's ClearUi (which also kills background PTYs). Documented limitation.
- `'new'` → `startTurn([{type:'text', text:'/new', text_elements:[]}])`. Server treats unknown slash commands as user text (matches TUI fallback behavior).
- `'quit'` → `window.close()`. In wry webview this may be a no-op (often blocked). Documented limitation; future work could surface a "use Cmd+Q" hint.

**`handleSlashCommand(cmd, args)`:**
```ts
const ctx: SlashCommandContext = {
  threadId,
  openSettings: (surface) => { setSettingsSurface(surface); setModal('settings'); },
  openModal: (m) => setModal(m),
  localAction: handleLocalAction,
};
const result = dispatchSlashCommand(cmd, args, ctx);
if (result.kind === 'sendTurn') {
  startTurn(result.input);
}
// openSettings/openModal/localAction already fired via ctx callbacks.
if (result.kind === 'notImplemented') {
  alert(result.message);  // simple stub; toast comes later
}
```

**Pass to Composer:** `onSlashCommand={handleSlashCommand}`

**Pass to SettingsModal:** `initialSurface={settingsSurface}`

**Scrollback wrapper:** `<div className="app-scrollback" key={clearKey}>` so bumping clearKey remounts it.

### App.test.tsx additions (5 cases)

1. `/skills` + Enter → SettingsModal open, Skills nav item has `settings-nav-item-active` class
2. `/model` + Enter → ModelPicker open (`data-testid="model-picker"`)
3. `/clear` + Enter → clearKey increments (test by spying on the Scrollback wrapper's key, or by observing that the conversation cells disappear after a `/clear` followed by a fake cell injection)
4. `/init` + Enter → `startTurn` called with payload containing "AGENTS.md"
5. `/diff` + Enter → `window.alert` called (stub `vi.stubGlobal('alert', vi.fn())` and assert)

### Verify

```bash
npx vitest run src/app/__tests__/App.test.tsx && npx tsc --noEmit
```

### Commit

```
feat(gui): wire slash commands into App + LocalAction handler

App.tsx routes onSlashCommand through dispatchSlashCommand, handles
sendTurn / openSettings / openModal / localAction / notImplemented.
LocalAction: clear (force-remount Scrollback), new (submit /new as
turn), quit (window.close). SettingsModal accepts initialSurface so
/skills etc. open on the right tab.

5 new App integration tests pass.
```

---

## Task 6: Styles + final regression

**Files:**
- Modify: `assets/src/styles.css`

### CSS to append

```css
/* ComposerPopup (subproject 5-E Stage 1). Anchored above the textarea. */
.composer { position: relative; }  /* anchor for .composer-popup */
.composer-popup { position: absolute; bottom: 100%; left: 0; right: 0; max-height: 240px; overflow-y: auto; background: var(--cell-bg); border: 1px solid var(--border); border-radius: 4px; z-index: 100; box-shadow: 0 4px 12px rgba(0,0,0,0.15); margin-bottom: 4px; }
.composer-popup-item { padding: 6px 12px; display: flex; flex-direction: column; gap: 2px; cursor: pointer; }
.composer-popup-item-active { background: var(--cell-bg-alt, rgba(127,127,127,0.10)); }
.composer-popup-item-name { font-size: 13px; font-weight: 500; }
.composer-popup-item-desc { font-size: 11px; color: var(--muted); }
.composer-popup-empty { padding: 8px 12px; font-size: 12px; color: var(--muted); font-style: italic; }
```

If `.composer` rule already exists elsewhere in styles.css, just add `position: relative` to it rather than re-declaring.

### Final verification

```bash
cd codex-rs/lemurclaw-gui/assets
npm test                       # expected: all green (~180 tests = 158 + ~22 new)
npx tsc --noEmit               # expected: clean
npx vitest run src/__tests__/build_smoke.test.ts   # catches asset path regressions
```

From repo root:
```bash
cd codex-rs && cargo check -p lemurclaw-gui   # sanity (no Rust changes)
```

### Commit

```
style(gui): composer popup styles

.composer gets position: relative to anchor the popup. .composer-popup
absolute-positioned above textarea with shadow + border. Active item
uses cell-bg-alt for highlight.
```

---

## Self-Review checklist

- ✅ **Spec coverage:** Implements `<SlashPopup>` from spec §4.2 (command popup 斜杠)
- ✅ **No wire protocol changes:** Catalog client-side hardcoded (verified `available_commands` doesn't exist on wire — corrects spec §4.2's incorrect assumption)
- ✅ **Stage 1 subset reasonable:** Covers all 4 actionable categories (sendTurn / openSettings / openModal / localAction) + 1 notImplemented stub
- ✅ **SettingsModal integration:** 8 openSettings commands map to its 9 surfaces (model has its own picker)
- ✅ **ModelPicker/ThemePicker integration:** openModal commands reuse ModalKind union
- ⚠️ **Sandbox limit:** Cannot verify popup visual (absolute positioning, scroll behavior) — jsdom tests cover interaction, filtering, keyboard. Visual e2e left to manual.
- ⚠️ **`/clear` simplified:** Force-remount, NOT equivalent to TUI's ClearUi (which also kills background PTYs)
- ⚠️ **`/quit` may no-op in wry:** `window.close()` is often blocked by webview hosts. Surfaced in code comment, not blocking.

## Out of scope (future stages)

- **Stage 2:** remaining ~30 commands (rename/archive/delete/resume/fork/logout/agent/side/btw/plan/goal/status/usage/debug-config/feedback/ide/import/approve/sandbox-*/keymap/vim/title/statusline/pets/ps/stop/copy/raw/rename/rollout/test-approval/debug-m-*)
- **Stage 3:** `<MentionPopup>` + `<FileSearchPopup>` (share `<ComposerPopup>` shell)
- **5-C:** `<DiffViewerModal>` — Stage 1's `/diff` returns notImplemented; 5-C implements the real viewer
