# Subproject 5-C: Diff Viewer Modal

> **For agentic workers:** Plan was approved via ExitPlanMode. Implement task-by-task, one commit per task. Write tool calls stay under ~150 lines — use Edit/Bash heredoc to append when growing files.

**Goal:** Add a full-screen `<DiffViewerModal>` to lemurclaw-gui that renders the current turn's aggregated unified diff with syntax highlighting. Covers spec §4.2 `<DiffBlock>` row.

**Three entry points:**
1. `/diff` slash command — change catalog's /diff dispatch from notImplemented → openModal('diff')
2. TopBar 📄 button (alongside 📜 transcript / 🎨 theme / ⚙ settings)
3. FileChangeCell header "view full diff" button (whole-cell fullscreen, coexists with per-file inline collapse)

**Out of scope (honest gap):** ApprovalCard view-diff button (TUI Ctrl+A equivalent). Reason: ApprovalCard's current props don't carry state; adding a button needs plumbing ConversationState (or a diffForItemId string) through App → ApprovalCard → FileChangeApproval — the plumbing exceeds 5-C's standalone value. Deferred.

**Data source:** Consume `turn/diff/updated` notification (exists on wire but reducer currently drops into `default` no-op branch). Core's `TurnDiffTracker` is the authoritative turn-level diff producer.

**Rendering:** `prism-react-renderer` for syntax highlighting. Requires hand-written unified-diff parser + diff-line color overlay (prism's diff grammar only colors +/- prefixes, doesn't tokenize content).

## Architecture

```
turn/diff/updated (ServerNotification)   ← wire exists, reducer ignores today
        │
        ▼
reducer: state.turnDiff = { turnId, diff }   ← new case
        │
        ▼
<DiffViewerModal> (new, based on <Modal> fullscreen shell)
   ├─ reads diff string from props
   └─ <DiffText> (new, parses unified diff + prism highlighting)
         ├─ parse: split on "diff --git" → per-file blocks
         ├─ per block: detect language from path extension
         ├─ <Highlight code={lines} language={lang}> from prism-react-renderer
         └─ overlay: +/- line background colors (green/red translucent)
```

## File Structure

**New files:**
- `assets/src/components/DiffText.tsx` — unified-diff parser + prism highlighting
- `assets/src/components/__tests__/DiffText.test.tsx`
- `assets/src/components/DiffViewerModal.tsx` — fullscreen modal shell
- `assets/src/components/__tests__/DiffViewerModal.test.tsx`

**Modified files:**
- `assets/src/viewModel/types.ts` — `ConversationState.turnDiff` field
- `assets/src/viewModel/reducer.ts` — `case 'turn/diff/updated'`
- `assets/src/viewModel/reducer.test.ts` — turnDiff reducer test
- `assets/src/components/cells/FileChangeCell.tsx` — header "view full diff" button
- `assets/src/components/cells/__tests__/FileChangeCell.test.tsx` — button test
- `assets/src/components/composer/slashCommands.ts` — /diff → openModal
- `assets/src/components/composer/slashCommandTypes.ts` — ModalKind += 'diff'
- `assets/src/components/TopBar.tsx` — 📄 button + onOpenDiff prop
- `assets/src/components/Scrollback.tsx` — prop-drill onViewDiff to CellRenderer
- `assets/src/app/App.tsx` — ModalKind += 'diff', render DiffViewerModal, handlers
- `assets/src/app/__tests__/App.test.tsx` — integration tests
- `assets/src/styles.css` — diff styles
- `assets/package.json` — prism-react-renderer dep

## Commit Plan

One commit per task:
1. Task 1 — prism dep + `<DiffText>` + tests
2. Task 2 — reducer consumes turn/diff/updated
3. Task 3 — `<DiffViewerModal>` shell
4. Task 4 — wire entry points (/diff + TopBar + FileChangeCell)
5. Task 5 — styles + final regression

---

## Task 1: Install prism-react-renderer + `<DiffText>`

**Files:**
- Modify: `assets/package.json` (add `prism-react-renderer`)
- Create: `assets/src/components/DiffText.tsx`
- Create: `assets/src/components/__tests__/DiffText.test.tsx`

### `<DiffText>` design

Props: `{ diff: string; className?: string }`

1. `parseDiff(diff)` — split on `diff --git a/... b/...` boundaries. Each block has `path` (from `b/` segment), `lang` (from extension), `lines` (body lines).
2. Per block render:
   - File header: path + line stats (`+N -M`)
   - Body: line-by-line, first-char-driven background color, prism `<Highlight>` on code content
3. Prism strategy: feed same-type contiguous lines (+段, -段, context段) to prism as one code blob, then split back into lines for color overlay. Avoids per-line tokenize breaking multi-line constructs.

**Simplifications:** No line numbers (TUI has them; prism overlay is complex). No theme switching (use prism's bundled `oneDark`/`oneLight`; decouple from CSS variables, unify later).

**Language detection:** Small extension→prism-lang map:
```ts
{ '.rs': 'rust', '.ts': 'typescript', '.tsx': 'tsx', '.js': 'javascript',
  '.py': 'python', '.md': 'markdown', '.css': 'css', '.html': 'markup',
  '.json': 'json', '.sh': 'bash', '.toml': 'toml' }
```
Unknown extension → `'none'` (prism renders plain).

**Line classification:**
- `+...` → add (`.diff-line-add`)
- `-...` → del (`.diff-line-del`)
- ` ...` (leading space) → context (`.diff-line-ctx`)
- `@@ ... @@` → hunk header (`.diff-line-hunk`)
- `diff --git` / `index ..` / `+++` / `---` → file meta (`.diff-line-meta`)

### Tests (7 cases)

1. Empty diff → empty placeholder
2. Single add-only diff → 1 block, header has path + `+N`
3. Mixed +/- diff → both line classes present
4. Multi-file diff → N blocks, each with own header
5. Unknown extension → no crash, uses `none`
6. `diff --git` boundary lines render as block separators
7. Each line has `data-testid="diff-line-<add|del|ctx|hunk|meta>"` for test assertability

### Verify

```bash
cd codex-rs/lemurclaw-gui/assets && npm install  # for prism-react-renderer
npx vitest run src/components/__tests__/DiffText.test.tsx
npx tsc --noEmit
```

### Commit

```
feat(gui): <DiffText> with prism-react-renderer syntax highlighting

Hand-written unified-diff parser splits input into per-file blocks,
classifies each line (+/-/context/hunk/meta), and runs prism on
same-type contiguous line groups for language-aware highlighting.
Extension→language map covers rust/ts/tsx/js/py/md/css/html/json/
sh/toml; unknown extensions fall back to plain text.

7 unit tests cover empty, single-file, multi-file, mixed, unknown-
extension, boundary, and per-line data-testid cases.
```

---

## Task 2: reducer consumes `turn/diff/updated`

**Files:**
- Modify: `assets/src/viewModel/types.ts`
- Modify: `assets/src/viewModel/reducer.ts`
- Modify: `assets/src/viewModel/reducer.test.ts`

### `types.ts` change

Add `turnDiff` field to `ConversationState`:

```ts
export interface ConversationState {
  // ... existing fields ...
  /** Latest turn-level unified diff from core's TurnDiffTracker. Null
   *  until the first turn/diff/updated notification arrives. */
  turnDiff: { turnId: string; diff: string } | null;
}

export const initialState: ConversationState = {
  // ... existing ...
  turnDiff: null,
};
```

### `reducer.ts` change

Add a new case before `default:`:

```ts
case 'turn/diff/updated':
  return {
    ...state,
    turnDiff: { turnId: n.params.turnId, diff: n.params.diff },
  };
```

### `reducer.test.ts` change

Add a test that dispatches `{ method: 'turn/diff/updated', params: { threadId: 't1', turnId: 'tu1', diff: 'sample diff text' } }` against `initialState`, asserts:
- `state.turnDiff` equals `{ turnId: 'tu1', diff: 'sample diff text' }`
- Other fields unchanged (deep equality with `initialState` except turnDiff)

### Verify

```bash
npx vitest run src/viewModel/reducer.test.ts && npx tsc --noEmit
```

### Commit

```
feat(gui): reducer consumes turn/diff/updated into ConversationState.turnDiff

New case in the reducer for the turn/diff/updated ServerNotification
(which was previously falling into the default no-op branch). Stores
the authoritative turn-level unified diff from core's TurnDiffTracker
as { turnId, diff } on ConversationState. This is the data source for
<DiffViewerModal> (Task 3-4).
```

---

## Task 3: `<DiffViewerModal>` shell

**Files:**
- Create: `assets/src/components/DiffViewerModal.tsx`
- Create: `assets/src/components/__tests__/DiffViewerModal.test.tsx`

### Props

```ts
interface Props {
  /** The unified diff text to render. If empty, modal shows empty-state. */
  diff: string;
  onClose: () => void;
}
```

### Implementation

Uses `<Modal>`'s `*ClassName` passthrough (the TranscriptPager pattern) to override defaults and make the modal near-fullscreen:

```tsx
import { Modal } from './Modal';
import { DiffText } from './DiffText';

export function DiffViewerModal({ diff, onClose }: Props) {
  return (
    <Modal
      title="diff"
      onClose={onClose}
      testId="diff-viewer-modal"
      overlayClassName="diff-viewer-overlay"
      contentClassName="diff-viewer-content"
      bodyClassName="diff-viewer-body"
    >
      {diff ? <DiffText diff={diff} /> : <div className="modal-empty">(no diff in this turn)</div>}
    </Modal>
  );
}
```

### Tests (4 cases)

1. Renders `data-testid="diff-viewer-modal"`
2. Non-empty diff → DiffText rendered (assert `.diff-text` class present)
3. Empty diff → `(no diff in this turn)` text present
4. Esc closes (inherited from Modal)

### Verify

```bash
npx vitest run src/components/__tests__/DiffViewerModal.test.tsx && npx tsc --noEmit
```

### Commit

```
feat(gui): <DiffViewerModal> full-screen diff viewer shell

Near-fullscreen modal (90vw × 90vh, z-index 1000 — same layer as
TranscriptPager) using <Modal>'s *ClassName passthrough. Renders
<DiffText> when diff is non-empty, empty-state placeholder otherwise.
```

---

## Task 4: Wire entry points (/diff + TopBar + FileChangeCell)

**Files:**
- Modify: `assets/src/components/composer/slashCommandTypes.ts` (ModalKind += 'diff')
- Modify: `assets/src/components/composer/slashCommands.ts` (/diff → openModal)
- Modify: `assets/src/components/TopBar.tsx` (📄 button + onOpenDiff prop)
- Modify: `assets/src/components/cells/FileChangeCell.tsx` (header button)
- Modify: `assets/src/components/cells/__tests__/FileChangeCell.test.tsx`
- Modify: `assets/src/components/Scrollback.tsx` (prop-drill onViewDiff)
- Modify: `assets/src/app/App.tsx` (ModalKind += 'diff', render DiffViewerModal, handlers)
- Modify: `assets/src/app/__tests__/App.test.tsx` (integration tests)

### slashCommandTypes.ts

```ts
export type ModalKind = 'model' | 'theme' | 'transcript' | 'settings' | 'diff';
```

### slashCommands.ts (/diff entry)

Replace the existing notImplemented stub:

```ts
{
  name: 'diff',
  description: 'Show git diff (including untracked files)',
  category: 'openModal',  // was 'notImplemented'
  dispatch: (_args, ctx) => { ctx.openModal('diff'); return { kind: 'openModal', modal: 'diff' }; },
},
```

Also update dispatch.test.ts: the `/diff` test now asserts openModal('diff') call instead of notImplemented message.

### TopBar.tsx

Add `onOpenDiff: () => void` prop and a 📄 button next to the existing 📜 icon button.

### FileChangeCell.tsx

Add optional `onViewDiff?: () => void` prop. Render a "view full diff" button in the header row (right side, after the file count). When prop is absent, no button (backward-compatible).

### Scrollback.tsx plumbing

Add optional `onViewDiff?: (cell: CellModel) => void` prop to Scrollback. Pass through CellRenderer → FileChangeCell. CellRenderer checks `cell.kind === 'fileChange'` before forwarding (other cell types ignore).

### App.tsx

```ts
type ModalKind = 'none' | 'model' | 'theme' | 'transcript' | 'settings' | 'diff';
const [diffSource, setDiffSource] = useState<string | null>(null);
// ...
<TopBar
  ...
  onOpenDiff={() => { setDiffSource(state.turnDiff?.diff ?? ''); setModal('diff'); }}
/>
<Scrollback
  state={state}
  onViewDiff={(cell) => {
    if (cell.kind !== 'fileChange') return;
    setDiffSource(cell.changes.map((c) => c.diff).join('\n'));
    setModal('diff');
  }}
/>
{modal === 'diff' && diffSource !== null && (
  <DiffViewerModal diff={diffSource} onClose={() => setModal('none')} />
)}
```

### Tests (6 cases in App.test.tsx)

1. `/diff` slash opens modal with turnDiff content (mock turnDiff in conversationState)
2. TopBar 📄 button (aria-label "diff") opens modal
3. FileChangeCell "view full diff" button opens modal with cell's diff
4. DiffViewerModal Esc closes
5. `/diff` with no turnDiff → modal opens but shows empty-state
6. Modal renders DiffText when diff non-empty (assert `.diff-text` inside modal)

### Verify

```bash
npm test && npx tsc --noEmit
```

### Commit

```
feat(gui): wire DiffViewerModal into /diff, TopBar, and FileChangeCell

Three entry points to open the diff viewer:
- /diff slash command now opens the modal (was notImplemented stub)
- TopBar gets a 📄 button alongside transcript/theme/settings
- FileChangeCell header gets a "view full diff" button showing that
  cell's changes

Scrollback prop-drills onViewDiff to CellRenderer to FileChangeCell.
App.tsx threads a diffSource string state to the modal. Modal renders
<DiffText> for the diff (Task 1) regardless of source.
```

---

## Task 5: Styles + final regression

**Files:**
- Modify: `assets/src/styles.css`

### CSS to append

```css
/* DiffViewerModal (subproject 5-C). */
.diff-viewer-overlay { z-index: 1000; }
.diff-viewer-content { width: 90vw; max-width: 1200px; max-height: 90vh; }
.diff-viewer-body { padding: 0; }

/* DiffText (subproject 5-C). */
.diff-text { font-family: var(--mono, monospace); font-size: 12px; }
.diff-file-block { border-bottom: 1px solid var(--border); }
.diff-file-header { padding: 8px 12px; background: var(--cell-bg-alt, rgba(127,127,127,0.05)); font-weight: 600; position: sticky; top: 0; z-index: 1; }
.diff-file-stats { font-weight: 400; margin-left: 12px; }
.diff-file-stats-add { color: #4caf50; }
.diff-file-stats-del { color: #f44336; }
.diff-line { display: flex; min-height: 18px; line-height: 18px; }
.diff-line-gutter { width: 24px; text-align: center; color: var(--muted); user-select: none; flex-shrink: 0; }
.diff-line-content { flex: 1; white-space: pre-wrap; word-break: break-all; padding-right: 12px; }
.diff-line-add { background: rgba(76, 175, 80, 0.12); }
.diff-line-del { background: rgba(244, 67, 54, 0.12); }
.diff-line-hunk { background: rgba(0, 150, 255, 0.08); color: var(--muted); }
.diff-line-meta { color: var(--muted); }

/* prism token overrides — let .diff-line-* backgrounds win */
.diff-line-content .token { background: transparent; }
```

### Final regression

```bash
cd codex-rs/lemurclaw-gui/assets
npm test                       # expected: ~210 tests green (200 + ~10 new)
npx tsc --noEmit               # clean
npx vitest run src/__tests__/build_smoke.test.ts   # verifies prism bundle is reasonable
```

From repo root: `cd codex-rs && cargo check -p lemurclaw-gui` (sanity, no Rust changes).

### Commit

```
style(gui): diff viewer + DiffText styles

.diff-viewer-* classes override <Modal> defaults for near-fullscreen
sizing (90vw × 90vh, z-index 1000). .diff-line-* classes drive the
+/-/context/hunk/meta line backgrounds. prism token backgrounds
overridden to transparent so diff-line backgrounds win.
```

---

## Self-Review checklist

- ✅ **Spec coverage:** spec §4.2 `<DiffBlock>` row (patch cell fullscreen diff)
- ✅ **Three entries cover main scenarios:** slash (keyboard), TopBar (quick access), FileChangeCell (in-place)
- ✅ **Authoritative data source:** turnDiff from core's TurnDiffTracker (not GUI-assembled)
- ✅ **Graceful degradation:** when turnDiff is absent (early turn / TopBar press), modal opens but shows empty-state (no crash)
- ⚠️ **No ApprovalCard button:** honest gap, plumbing cost exceeds 5-C value
- ⚠️ **Sandbox limit:** syntax-highlighting visual quality can't be verified in jsdom (only structure + class assertions); real visual e2e left to manual
- ⚠️ **prism dep added:** bundle +50-80KB expected, verified via build_smoke total-size sanity check

## Out of scope

- **ApprovalCard view-diff button** (TUI Ctrl+A equivalent) — plumbing-heavy, deferred
- **MentionPopup + FileSearchPopup** (5-E Stage 3)
- **Line numbers** (TUI has; prism overlay complex; skip)
- **Theme switching** (use prism's bundled themes; unify with CSS variables later)
