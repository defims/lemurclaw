# SettingsModal Implementation Plan (LemurClaw Subproject 5-A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a unified `<SettingsModal>` to the lemurclaw-gui frontend that surfaces codex app-server config (permissions, keymap, memories, skills, hooks, mcp, apps, plugins, experimental features, statusline/title) behind a single gear icon, and refactor the three existing modal pickers onto a shared `<Modal>` shell first.

**Architecture:** A shared `<Modal>` wrapper owns the overlay/Esc/backdrop/close-button shell that `ModelPicker`, `ThemePicker`, and `TranscriptPager` each duplicate today. On top of that, a `<SettingsModal>` hosts a left-nav of surfaces and a right pane that swaps per surface. Three reusable primitives — `<Modal>`, `<SettingsListPicker>`, `<SettingsForm>` — cover all surfaces so each surface file stays small and declarative.

**Tech Stack:** React 18 + TypeScript, Vitest 2 + @testing-library/react 16 + jsdom, ts-rs generated types under `assets/src/types/v2/`, JSON-RPC over the wry IPC bridge via `sendRequest` from `assets/src/transport.ts`.

**Repo root for all paths below:** `codex-rs/lemurclaw-gui/`. All file paths are relative to that root unless noted.

**Conventions (from `AGENTS.md` + existing GUI code):**
- Test files live in `assets/src/components/__tests__/` (sibling `__tests__` dir), matching `ModelPicker.test.tsx` etc.
- Tests mock `../../transport` with `vi.mock` and assert on rendered output + click handlers — **no snapshot tests** in the GUI (existing tests use explicit assertions; the TUI insta convention does not apply here).
- `sendRequest<T>(method, params)` is the typed request channel; mock it with `vi.mocked(sendRequest).mockResolvedValue(...)`.
- Modal Esc handlers are **window** listeners (jsdom focus is unreliable) — dispatch `fireEvent.keyDown(window, { key: 'Escape' })`.
- Run frontend tests from `codex-rs/lemurclaw-gui/assets/`: `npm test` (vitest run) or `npx vitest run path/to/test.test.tsx`.
- Run `npx tsc --noEmit` from `codex-rs/lemurclaw-gui/assets/` to typecheck before committing.
- Do NOT touch Rust. All changes are under `codex-rs/lemurclaw-gui/assets/src/`.

---

## File Structure

**New files:**
- `assets/src/components/Modal.tsx` — shared overlay shell (Task 1)
- `assets/src/components/Modal.test.tsx` — Modal shell tests (Task 1)
- `assets/src/components/SettingsListPicker.tsx` — generic list+states (Task 2)
- `assets/src/components/SettingsListPicker.test.tsx` (Task 2)
- `assets/src/components/settings/SettingsModal.tsx` — left-nav shell (Task 3)
- `assets/src/components/settings/SettingsModal.test.tsx` (Task 3)
- `assets/src/components/settings/PermissionsPanel.tsx` (Task 4)
- `assets/src/components/settings/HooksPanel.tsx` (Task 4)
- `assets/src/components/settings/McpPanel.tsx` (Task 4)
- `assets/src/components/settings/SkillsPanel.tsx` (Task 5)
- `assets/src/components/settings/AppsPanel.tsx` (Task 5)
- `assets/src/components/settings/PluginsPanel.tsx` (Task 5)
- `assets/src/components/settings/ExperimentalPanel.tsx` (Task 6)
- `assets/src/components/settings/SettingsForm.tsx` — config-key editor (Task 7)
- `assets/src/components/settings/ConfigFormPanel.tsx` — generic wrapper for keymap/memories/statusline/title (Task 7)
- Per-panel test files under `assets/src/components/settings/__tests__/`

**Modified files:**
- `assets/src/components/ModelPicker.tsx` — use `<Modal>` (Task 1)
- `assets/src/components/ThemePicker.tsx` — use `<Modal>` (Task 1)
- `assets/src/components/TranscriptPager.tsx` — use `<Modal>` (Task 1)
- `assets/src/components/TopBar.tsx` — add gear icon + `onOpenSettings` prop (Task 3)
- `assets/src/app/App.tsx` — add `'settings'` to `ModalKind`, wire open/close (Task 3)
- `assets/src/styles.css` — add settings-modal layout classes (Task 3, extended per batch)

**Each task = one commit.** Tasks 1, 2, 3 are foundational and land first; Tasks 4-7 are independent batches that can land in any order after 3; Task 8 is final review + branch finishing.

---

## Task 1: Shared `<Modal>` wrapper + migrate three existing modals

**Goal:** Extract the duplicated overlay/Esc/backdrop/close-button shell (currently copied across `ModelPicker`, `ThemePicker`, `TranscriptPager`) into one `<Modal>` component. **Zero behavior change** — existing tests must keep passing unmodified. This is a pure refactor that unlocks Tasks 2-7.

**Files:**
- Create: `assets/src/components/Modal.tsx`
- Create: `assets/src/components/Modal.test.tsx`
- Modify: `assets/src/components/ModelPicker.tsx`
- Modify: `assets/src/components/ThemePicker.tsx`
- Modify: `assets/src/components/TranscriptPager.tsx`

- [ ] **Step 1: Write failing test for `<Modal>`**

Create `assets/src/components/Modal.test.tsx`:

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Modal } from '../Modal';

describe('Modal', () => {
  it('renders title and children', () => {
    render(
      <Modal title="select model" onClose={vi.fn()}>
        <div>body content</div>
      </Modal>,
    );
    expect(screen.getByText('select model')).toBeInTheDocument();
    expect(screen.getByText('body content')).toBeInTheDocument();
  });

  it('Esc closes via window listener', () => {
    const onClose = vi.fn();
    render(<Modal title="t" onClose={onClose}><div /></Modal>);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('backdrop click closes', () => {
    const onClose = vi.fn();
    const { container } = render(<Modal title="t" onClose={onClose}><div /></Modal>);
    fireEvent.click(container.querySelector('.modal-overlay')!);
    expect(onClose).toHaveBeenCalled();
  });

  it('click inside content does not close', () => {
    const onClose = vi.fn();
    render(
      <Modal title="t" onClose={onClose}>
        <button>inside</button>
      </Modal>,
    );
    fireEvent.click(screen.getByText('inside'));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('close button closes', () => {
    const onClose = vi.fn();
    render(<Modal title="t" onClose={onClose}><div /></Modal>);
    fireEvent.click(screen.getByLabelText('close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('applies data-testid when provided', () => {
    render(
      <Modal title="t" onClose={vi.fn()} testId="model-picker">
        <div />
      </Modal>,
    );
    expect(screen.getByTestId('model-picker')).toBeInTheDocument();
  });

  it('applies wide class when wide=true (settings sizing)', () => {
    const { container } = render(
      <Modal title="t" onClose={vi.fn()} wide>
        <div />
      </Modal>,
    );
    expect(container.querySelector('.modal-content')).toHaveClass('modal-content-wide');
  });

  it('passes through surface-specific className props (for full-screen surfaces)', () => {
    const { container } = render(
      <Modal
        title="t"
        onClose={vi.fn()}
        overlayClassName="transcript-pager-overlay"
        contentClassName="transcript-pager-content"
        headerClassName="transcript-pager-header"
        titleClassName="transcript-pager-title"
        closeClassName="transcript-pager-close"
        bodyClassName="transcript-pager-body"
      >
        <div />
      </Modal>,
    );
    // Each element gets BOTH the base modal-* class and the surfaced class.
    expect(container.querySelector('.modal-overlay')).toHaveClass('transcript-pager-overlay');
    expect(container.querySelector('.modal-content')).toHaveClass('transcript-pager-content');
    expect(container.querySelector('.modal-header')).toHaveClass('transcript-pager-header');
    expect(container.querySelector('.modal-title')).toHaveClass('transcript-pager-title');
    expect(container.querySelector('.modal-close')).toHaveClass('transcript-pager-close');
    expect(container.querySelector('.modal-body')).toHaveClass('transcript-pager-body');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/Modal.test.tsx`
Expected: FAIL — `Cannot find module '../Modal'` or similar.

- [ ] **Step 3: Implement `<Modal>`**

Create `assets/src/components/Modal.tsx`:

```tsx
import { useEffect, type ReactNode } from 'react';

interface Props {
  /** Title shown in the header bar. */
  title: ReactNode;
  /** Close handler. Fired on Esc, backdrop click, and ✕ button. */
  onClose: () => void;
  /** Modal body. */
  children: ReactNode;
  /** Optional `data-testid` for the overlay root (existing pickers set one). */
  testId?: string;
  /** When true, adds `modal-content-wide` class for settings-style width.
   *  Pickers stay narrow (default). */
  wide?: boolean;
  /** Extra class on the overlay div (e.g. `transcript-pager-overlay` so a
   *  full-screen surface can keep its bespoke sizing + z-index while sharing
   *  the Esc/backdrop/✕ logic). */
  overlayClassName?: string;
  /** Extra class on the content div (e.g. `transcript-pager-content`). */
  contentClassName?: string;
  /** Extra class on the header (e.g. `transcript-pager-header`). */
  headerClassName?: string;
  /** Extra class on the title span (e.g. `transcript-pager-title`). */
  titleClassName?: string;
  /** Extra class on the close button (e.g. `transcript-pager-close`). */
  closeClassName?: string;
  /** Extra class on the body div (e.g. `transcript-pager-body`). */
  bodyClassName?: string;
}

/** Shared modal shell: fixed overlay, centered content card, header with title
 *  + ✕ close button, scrollable body. Owns the three close vectors (Esc window
 *  listener, backdrop click, ✕ button) that ModelPicker/ThemePicker/
 *  TranscriptPager previously duplicated.
 *
 *  The Esc handler is a window listener (not on the content node) so it fires
 *  regardless of focus — matches the pre-refactor behavior the existing tests
 *  assert on (`fireEvent.keyDown(window, { key: 'Escape' })`).
 *
 *  Surface-specific sizing/classes are kept via the `*ClassName` props —
 *  TranscriptPager uses them to stay full-screen (its existing test queries
 *  `.transcript-pager-overlay` and expects 90vw × 90vh + z-index 1000);
 *  pickers leave them off and get the default 480px card (`wide` → 720px). */
export function Modal({ title, onClose, children, testId, wide, overlayClassName, contentClassName, headerClassName, titleClassName, closeClassName, bodyClassName }: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const overlayCls = ['modal-overlay', overlayClassName].filter(Boolean).join(' ');
  const contentCls = ['modal-content', wide ? 'modal-content-wide' : null, contentClassName].filter(Boolean).join(' ');
  const headerCls = ['modal-header', headerClassName].filter(Boolean).join(' ');
  const titleCls = ['modal-title', titleClassName].filter(Boolean).join(' ');
  const closeCls = ['modal-close', closeClassName].filter(Boolean).join(' ');
  const bodyCls = ['modal-body', bodyClassName].filter(Boolean).join(' ');

  return (
    <div className={overlayCls} data-testid={testId} onClick={onClose}>
      <div className={contentCls} onClick={(e) => e.stopPropagation()}>
        <header className={headerCls}>
          <span className={titleCls}>{title}</span>
          <button className={closeCls} onClick={onClose} aria-label="close">✕</button>
        </header>
        <div className={bodyCls}>{children}</div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run `<Modal>` test to verify it passes**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/Modal.test.tsx`
Expected: PASS (7 tests).

- [ ] **Step 5: Add `modal-content-wide` style**

Edit `assets/src/styles.css`. Find the existing `.modal-content` line and add a wide variant immediately after it:

```css
.modal-content-wide { width: 720px; max-width: 92vw; max-height: 85vh; }
```

- [ ] **Step 6: Migrate `ModelPicker` onto `<Modal>`**

Replace the entire file `assets/src/components/ModelPicker.tsx` with:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../transport';
import { Modal } from './Modal';
import type { Model } from '../types/v2';
import type { ModelListResponse } from '../types/v2/ModelListResponse';

interface Props {
  /** Active thread id. Sent as thread/start's threadId when switching. */
  threadId: string | null;
  /** Currently selected model id (for highlight). Optional. */
  currentModel?: string | null;
  onClose: () => void;
  /** Send a turn/start ClientRequest via sendRequest (with model override) and
   *  fold the response's cwd/model into state. */
  startTurn: (input: unknown[], modelOverride?: string) => Promise<void>;
}

interface LoadState {
  loading: boolean;
  error: string | null;
  models: Model[];
}

/** Modal model picker. On open, calls `model/list` to enumerate available
 *  models; selecting one fires a `turn/start` with `model` override on the
 *  active thread (codex doesn't have a dedicated "switch model mid-thread"
 *  method — the override takes effect on the next turn). */
export function ModelPicker({ threadId, currentModel, onClose, startTurn }: Props) {
  const [state, setState] = useState<LoadState>({ loading: true, error: null, models: [] });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, models: [] });
    sendRequest<ModelListResponse>('model/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, models: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), models: [] });
      });
    return () => { cancelled = true; };
  }, []);

  const handlePick = (model: Model) => {
    if (!threadId) return;
    // Switch model via next-turn override. Empty input = no new user message;
    // codex treats this as a no-op turn, model still takes effect next turn.
    startTurn([{ type: 'text', text: '', text_elements: [] }], model.id);
    onClose();
  };

  return (
    <Modal title="select model" onClose={onClose} testId="model-picker">
      {state.loading && <div className="modal-loading">loading…</div>}
      {state.error && <div className="modal-error">failed: {state.error}</div>}
      {!state.loading && !state.error && state.models.length === 0 && (
        <div className="modal-empty">no models configured</div>
      )}
      {!state.loading && !state.error && state.models.length > 0 && (
        <ul className="model-list">
          {state.models.map((m) => (
            <li
              key={m.id}
              className={`model-item${m.id === currentModel ? ' model-item-active' : ''}`}
            >
              <button onClick={() => handlePick(m)} className="model-item-button" disabled={!threadId}>
                <span className="model-item-name">{m.displayName || m.id}</span>
                <span className="model-item-id">{m.id}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}
```

- [ ] **Step 7: Migrate `ThemePicker` onto `<Modal>`**

Replace the entire file `assets/src/components/ThemePicker.tsx` with:

```tsx
import { THEMES, type ThemeName } from '../themes';
import { Modal } from './Modal';

interface Props {
  current: ThemeName;
  onPick: (t: ThemeName) => void;
  onClose: () => void;
}

/** Modal theme picker. Lists THEMES, highlights the current one, calls onPick
 *  when a theme is selected. The caller (App.tsx) wires onPick to useTheme's
 *  setTheme + closes the modal. */
export function ThemePicker({ current, onPick, onClose }: Props) {
  return (
    <Modal title="select theme" onClose={onClose} testId="theme-picker">
      <ul className="theme-list">
        {THEMES.map((t) => (
          <li key={t.name} className={`theme-item${t.name === current ? ' theme-item-active' : ''}`}>
            <button onClick={() => onPick(t.name)} className="theme-item-button">
              <span className="theme-item-name">{t.label}</span>
              <span className="theme-item-desc">{t.description}</span>
            </button>
          </li>
        ))}
      </ul>
    </Modal>
  );
}
```

- [ ] **Step 8: Migrate `TranscriptPager` onto `<Modal>`**

Replace the entire file `assets/src/components/TranscriptPager.tsx` with:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../transport';
import { Modal } from './Modal';
import type { Thread } from '../types/v2';
import type { CellModel } from '../viewModel/types';
import { threadItemToCell } from '../viewModel/reducer';
import { CellRenderer, cellKey } from './Scrollback';

interface Props {
  /** Thread to load. The pager fetches its turns on mount. */
  threadId: string;
  /** Close handler (Esc or backdrop click). */
  onClose: () => void;
}

interface ReadState {
  loading: boolean;
  error: string | null;
  cells: CellModel[];
  thread: Thread | null;
}

/** Full-screen transcript pager (codex TUI's Ctrl+T equivalent).
 *
 *  Loads the thread's full turn history via `thread/read { includeTurns: true }`
 *  and renders all items flat (no turn boundaries) using the same cell
 *  components as Scrollback (via the shared CellRenderer). Read-only — no
 *  input, no approvals. */
export function TranscriptPager({ threadId, onClose }: Props) {
  const [state, setState] = useState<ReadState>({ loading: true, error: null, cells: [], thread: null });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, cells: [], thread: null });
    sendRequest<{ thread: Thread }>('thread/read', { threadId, includeTurns: true })
      .then((resp) => {
        if (cancelled) return;
        const cells = resp.thread.turns.flatMap((t) => t.items.map(threadItemToCell));
        setState({ loading: false, error: null, cells, thread: resp.thread });
      })
      .catch((e) => {
        if (cancelled) return;
        setState({ loading: false, error: e instanceof Error ? e.message : String(e), cells: [], thread: null });
      });
    return () => { cancelled = true; };
  }, [threadId]);

  return (
    <Modal
      title={`transcript · ${state.thread?.name ?? state.thread?.preview ?? threadId}`}
      onClose={onClose}
      testId="transcript-pager"
      overlayClassName="transcript-pager-overlay"
      contentClassName="transcript-pager-content"
      headerClassName="transcript-pager-header"
      titleClassName="transcript-pager-title"
      closeClassName="transcript-pager-close"
      bodyClassName="transcript-pager-body"
    >
      {state.loading && <div className="transcript-pager-loading">loading…</div>}
      {state.error && <div className="transcript-pager-error">failed: {state.error}</div>}
      {!state.loading && !state.error && state.cells.length === 0 && (
        <div className="transcript-pager-empty">(no items in transcript)</div>
      )}
      {!state.loading && !state.error && state.cells.length > 0 && (
        <div className="transcript-pager-cells">
          {state.cells.map((c) => <CellRenderer key={cellKey(c)} cell={c} />)}
        </div>
      )}
    </Modal>
  );
}
```

TranscriptPager keeps ALL its bespoke `transcript-pager-*` classes via the `*ClassName` props — each element gets both the base `modal-*` class and the surfaced class. The existing test queries `.transcript-pager-overlay` (which still exists on the overlay div) and the full-screen sizing (90vw × 90vh, z-index 1000) is preserved because `.transcript-pager-overlay` / `.transcript-pager-content` are defined later in styles.css and win specificity.

- [ ] **Step 9: Run the full picker test suites — they MUST pass unmodified**

The existing `ModelPicker.test.tsx` and `ThemePicker.test.tsx` assert on `.modal-overlay` and window Esc — unchanged. `TranscriptPager.test.tsx` asserts on `.transcript-pager-overlay` (still present via the className passthrough) and window Esc — also unchanged.

Run:
```bash
cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/__tests__/ModelPicker.test.tsx src/components/__tests__/ThemePicker.test.tsx src/components/__tests__/TranscriptPager.test.tsx src/components/Modal.test.tsx
```
Expected: PASS (all tests across all 4 files).

- [ ] **Step 10: Typecheck + run the whole frontend suite**

```bash
cd codex-rs/lemurclaw-gui/assets && npx tsc --noEmit && npm test
```
Expected: `tsc` clean; all tests pass (the GUI frontend test suite).

- [ ] **Step 11: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/Modal.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/Modal.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/ModelPicker.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/ThemePicker.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/TranscriptPager.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "refactor(gui): extract shared <Modal> wrapper, migrate 3 pickers

ModelPicker/ThemePicker/TranscriptPager each duplicated the same
overlay/Esc/backdrop/close-button shell. Extract into <Modal> and
migrate all three. Zero behavior change — existing picker tests pass
unmodified. Adds a wide variant (modal-content-wide) for the upcoming
SettingsModal (subproject 5-A)."
```

---

## Task 2: `<SettingsListPicker>` — generic list+states primitive

**Goal:** A reusable component that renders a list of items with loading/empty/error states, an active-item highlight, and an optional per-item action. Every read-only list surface (permissions, hooks, mcp) and editable list surface (skills, apps, plugins) builds on this. Pure presentational — the panel composes it and supplies data + handlers.

**Files:**
- Create: `assets/src/components/SettingsListPicker.tsx`
- Create: `assets/src/components/SettingsListPicker.test.tsx`

- [ ] **Step 1: Write failing test**

Create `assets/src/components/SettingsListPicker.test.tsx`:

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SettingsListPicker } from '../SettingsListPicker';

interface Item { id: string; label: string; sub?: string; disabled?: boolean }

describe('SettingsListPicker', () => {
  it('renders loading state', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: true, error: null, items: [] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        renderSub={(i) => i.sub}
        isDisabled={(i) => i.disabled ?? false}
      />,
    );
    expect(screen.getByText('loading…')).toBeInTheDocument();
  });

  it('renders error state', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: 'oops', items: [] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
      />,
    );
    expect(screen.getByText(/failed: oops/)).toBeInTheDocument();
  });

  it('renders empty state', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        emptyText="nothing here"
      />,
    );
    expect(screen.getByText('nothing here')).toBeInTheDocument();
  });

  it('renders items and highlights active', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [
          { id: 'a', label: 'Alpha' },
          { id: 'b', label: 'Beta' },
        ] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        activeId="b"
      />,
    );
    expect(screen.getByText('Alpha')).toBeInTheDocument();
    expect(screen.getByText('Beta')).toBeInTheDocument();
    expect(screen.getByText('Beta').closest('.settings-list-item')).toHaveClass('settings-list-item-active');
  });

  it('fires onActivate when an enabled item is clicked', () => {
    const onActivate = vi.fn();
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [{ id: 'a', label: 'Alpha' }] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        onActivate={onActivate}
      />,
    );
    fireEvent.click(screen.getByText('Alpha'));
    expect(onActivate).toHaveBeenCalledWith({ id: 'a', label: 'Alpha' });
  });

  it('does not fire onActivate when disabled', () => {
    const onActivate = vi.fn();
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [{ id: 'a', label: 'Alpha', disabled: true }] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        isDisabled={(i) => i.disabled ?? false}
        onActivate={onActivate}
      />,
    );
    fireEvent.click(screen.getByText('Alpha'));
    expect(onActivate).not.toHaveBeenCalled();
  });

  it('renders trailing action button when renderAction provided', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [{ id: 'a', label: 'Alpha' }] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        renderAction={(i) => (
          <button data-testid={`act-${i.id}`} onClick={vi.fn()}>uninstall</button>
        )}
      />,
    );
    expect(screen.getByTestId('act-a')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/SettingsListPicker.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `<SettingsListPicker>`**

Create `assets/src/components/SettingsListPicker.tsx`:

```tsx
import type { ReactNode } from 'react';

/** Shared load/error/empty/list render for settings panels backed by a list
 *  RPC. Each panel owns the fetch useEffect (so it can vary method/params) and
 *  hands the resulting state here. Generic over the item type so panels don't
 *  need adapter layers. */
export interface LoadState<T> {
  loading: boolean;
  error: string | null;
  items: T[];
}

interface Props<T> {
  state: LoadState<T>;
  /** Stable key for React list rendering. */
  getId: (item: T) => string;
  /** Primary label text for each row. */
  renderLabel: (item: T) => ReactNode;
  /** Optional secondary line (id, description, etc). */
  renderSub?: (item: T) => ReactNode;
  /** Optional trailing action (uninstall, install, …). */
  renderAction?: (item: T) => ReactNode;
  /** Whether the row is non-selectable. Defaults to false. */
  isDisabled?: (item: T) => boolean;
  /** Currently-active row id (highlight). */
  activeId?: string | null;
  /** Fired when an enabled row is clicked. Absent for read-only panels. */
  onActivate?: (item: T) => void;
  /** Override the default "(empty)" empty-state copy. */
  emptyText?: string;
}

export function SettingsListPicker<T>({
  state,
  getId,
  renderLabel,
  renderSub,
  renderAction,
  isDisabled,
  activeId,
  onActivate,
  emptyText = '(empty)',
}: Props<T>) {
  if (state.loading) return <div className="modal-loading">loading…</div>;
  if (state.error) return <div className="modal-error">failed: {state.error}</div>;
  if (state.items.length === 0) return <div className="modal-empty">{emptyText}</div>;

  return (
    <ul className="settings-list">
      {state.items.map((item) => {
        const id = getId(item);
        const disabled = isDisabled ? isDisabled(item) : false;
        const active = id === activeId;
        return (
          <li
            key={id}
            className={`settings-list-item${active ? ' settings-list-item-active' : ''}${disabled ? ' settings-list-item-disabled' : ''}`}
          >
            <button
              className="settings-list-item-button"
              onClick={() => onActivate?.(item)}
              disabled={disabled || !onActivate}
            >
              <span className="settings-list-item-label">{renderLabel(item)}</span>
              {renderSub && <span className="settings-list-item-sub">{renderSub(item)}</span>}
            </button>
            {renderAction && <span className="settings-list-item-action">{renderAction(item)}</span>}
          </li>
        );
      })}
    </ul>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/SettingsListPicker.test.tsx`
Expected: PASS (7 tests).

- [ ] **Step 5: Add settings-list styles**

Append to `assets/src/styles.css`:

```css
.settings-list { list-style: none; margin: 0; padding: 0; }
.settings-list-item { display: flex; align-items: center; gap: 8px; border-bottom: 1px solid var(--border); }
.settings-list-item-active { background: var(--cell-bg-alt, rgba(127,127,127,0.08)); }
.settings-list-item-disabled { opacity: 0.5; }
.settings-list-item-button { flex: 1; display: flex; flex-direction: column; gap: 2px; text-align: left; background: none; border: none; padding: 8px 12px; cursor: pointer; color: inherit; }
.settings-list-item-button:disabled { cursor: default; }
.settings-list-item-label { font-size: 13px; }
.settings-list-item-sub { font-size: 11px; color: var(--muted); }
.settings-list-item-action { padding-right: 12px; }
```

- [ ] **Step 6: Typecheck**

```bash
cd codex-rs/lemurclaw-gui/assets && npx tsc --noEmit
```
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/SettingsListPicker.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/SettingsListPicker.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): add <SettingsListPicker> primitive

Generic list+loading/error/empty/active renderer that settings panels
(Task 4-5) compose with their own fetch effect and item shape. Generic
over item type; supports optional trailing action button and disabled
rows. Adds .settings-list styles."
```

---

## Task 3: `<SettingsModal>` shell + gear icon in TopBar

**Goal:** Build the settings modal shell — a wide `<Modal>` with a left-nav of surface names and an empty right pane. Add a ⚙ gear button to TopBar. Wire open/close in App.tsx via a new `'settings'` ModalKind. **No surfaces yet** — Task 4-7 fill them in. The shell must render and the gear must open it; the right pane shows placeholder text per surface.

**Files:**
- Create: `assets/src/components/settings/SettingsModal.tsx`
- Create: `assets/src/components/settings/SettingsModal.test.tsx`
- Modify: `assets/src/components/TopBar.tsx`
- Modify: `assets/src/app/App.tsx`
- Modify: `assets/src/styles.css`

- [ ] **Step 1: Write failing test**

Create `assets/src/components/settings/SettingsModal.test.tsx`:

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsModal } from '../SettingsModal';

describe('SettingsModal', () => {
  it('renders surface list in left nav', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByText('Permissions')).toBeInTheDocument();
    expect(screen.getByText('Keymap')).toBeInTheDocument();
    expect(screen.getByText('Skills')).toBeInTheDocument();
    expect(screen.getByText('Plugins')).toBeInTheDocument();
    expect(screen.getByText('Experimental')).toBeInTheDocument();
  });

  it('defaults to first surface selected with its placeholder', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    // First surface is "Permissions"
    expect(screen.getByText('Permissions').closest('.settings-nav-item')).toHaveClass('settings-nav-item-active');
    expect(screen.getByText(/permissions panel/i)).toBeInTheDocument();
  });

  it('clicking a surface swaps the right pane placeholder', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Plugins'));
    expect(screen.getByText('Plugins').closest('.settings-nav-item')).toHaveClass('settings-nav-item-active');
    expect(screen.getByText(/plugins panel/i)).toBeInTheDocument();
  });

  it('Esc closes (via shared <Modal>)', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('close button closes', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} />);
    fireEvent.click(screen.getByLabelText('close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('has data-testid settings-modal on overlay', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByTestId('settings-modal')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/SettingsModal.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `<SettingsModal>` shell**

Create `assets/src/components/settings/SettingsModal.tsx`:

```tsx
import { useState } from 'react';
import { Modal } from '../Modal';

/** Identifiers for the settings surfaces. Each one maps to a panel file
 *  populated in Tasks 4-7. Order here = order in the left nav. */
export type SettingsSurface =
  | 'permissions'
  | 'keymap'
  | 'memories'
  | 'skills'
  | 'hooks'
  | 'mcp'
  | 'apps'
  | 'plugins'
  | 'experimental'
  | 'statusline';

interface SurfaceDef {
  key: SettingsSurface;
  label: string;
}

const SURFACES: SurfaceDef[] = [
  { key: 'permissions', label: 'Permissions' },
  { key: 'keymap', label: 'Keymap' },
  { key: 'memories', label: 'Memories' },
  { key: 'skills', label: 'Skills' },
  { key: 'hooks', label: 'Hooks' },
  { key: 'mcp', label: 'MCP' },
  { key: 'apps', label: 'Apps' },
  { key: 'plugins', label: 'Plugins' },
  { key: 'experimental', label: 'Experimental' },
  { key: 'statusline', label: 'Status line' },
];

interface Props {
  onClose: () => void;
}

/** Settings modal shell: left-nav of surfaces + a right pane that renders the
 *  active surface's panel. Surfaces are added incrementally — until a panel
 *  exists, the right pane shows a placeholder naming the surface. Esc, backdrop
 *  click, and ✕ close are inherited from <Modal>. */
export function SettingsModal({ onClose }: Props) {
  const [surface, setSurface] = useState<SettingsSurface>('permissions');

  return (
    <Modal title="settings" onClose={onClose} testId="settings-modal" wide>
      <div className="settings-modal-body">
        <nav className="settings-nav" aria-label="settings sections">
          {SURFACES.map((s) => (
            <button
              key={s.key}
              className={`settings-nav-item${s.key === surface ? ' settings-nav-item-active' : ''}`}
              onClick={() => setSurface(s.key)}
            >
              {s.label}
            </button>
          ))}
        </nav>
        <div className="settings-pane" data-testid={`settings-pane-${surface}`}>
          <Placeholder surface={surface} />
        </div>
      </div>
    </Modal>
  );
}

/** Placeholder shown until each surface's real panel lands (Tasks 4-7). Each
 *  real panel replaces the matching case. */
function Placeholder({ surface }: { surface: SettingsSurface }) {
  return <div className="modal-empty">{surface} panel — coming soon</div>;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/SettingsModal.test.tsx`
Expected: PASS (6 tests).

- [ ] **Step 5: Add settings-modal layout styles**

Append to `assets/src/styles.css`:

```css
.settings-modal-body { display: flex; min-height: 420px; gap: 0; margin: -12px; }
.settings-nav { width: 160px; flex-shrink: 0; border-right: 1px solid var(--border); display: flex; flex-direction: column; padding: 4px 0; }
.settings-nav-item { text-align: left; background: none; border: none; padding: 6px 12px; cursor: pointer; color: var(--muted); font-size: 13px; }
.settings-nav-item:hover { background: var(--cell-bg-alt, rgba(127,127,127,0.06)); color: inherit; }
.settings-nav-item-active { color: inherit; font-weight: 600; background: var(--cell-bg-alt, rgba(127,127,127,0.10)); }
.settings-pane { flex: 1; padding: 12px; overflow-y: auto; }
```

- [ ] **Step 6: Add gear icon to TopBar**

Edit `assets/src/components/TopBar.tsx`. Add an `onOpenSettings` prop and a gear button:

```tsx
interface Props {
  cwd: string | null;
  model: string | null;
  onOpenModelPicker: () => void;
  onOpenThemePicker: () => void;
  onOpenTranscript: () => void;
  onOpenSettings: () => void;
}

export function TopBar({ cwd, model, onOpenModelPicker, onOpenThemePicker, onOpenTranscript, onOpenSettings }: Props) {
  return (
    <header className="app-topbar" data-testid="topbar">
      <span className="topbar-cwd">{cwd ?? '(no cwd)'}</span>
      <button className="topbar-button" onClick={onOpenModelPicker}>
        <span className="topbar-model">{model ?? '(no model)'}</span> ⏷
      </button>
      <div className="topbar-spacer" />
      <button className="topbar-icon-button" onClick={onOpenTranscript} aria-label="transcript" title="transcript (Ctrl+T)">
        📜
      </button>
      <button className="topbar-icon-button" onClick={onOpenThemePicker} aria-label="theme" title="theme">
        🎨
      </button>
      <button className="topbar-icon-button" onClick={onOpenSettings} aria-label="settings" title="settings">
        ⚙
      </button>
    </header>
  );
}
```

Keep the existing JSDoc comment block above the function (update only the prop interface and add the gear button — minimal diff).

- [ ] **Step 7: Wire `'settings'` into App.tsx**

Edit `assets/src/app/App.tsx`:

1. Change the `ModalKind` type:
```tsx
type ModalKind = 'none' | 'model' | 'theme' | 'transcript' | 'settings';
```

2. Add `onOpenSettings` to the `<TopBar>` invocation:
```tsx
        <TopBar
          cwd={state.cwd}
          model={state.currentModel}
          onOpenModelPicker={() => setModal('model')}
          onOpenThemePicker={() => setModal('theme')}
          onOpenTranscript={() => setModal('transcript')}
          onOpenSettings={() => setModal('settings')}
        />
```

3. Import and render `<SettingsModal>` (next to the other modal renders, below `ThemePicker`):
```tsx
import { SettingsModal } from '../components/settings/SettingsModal';
```
…and at the bottom of the `<Onboarding>` tree:
```tsx
      {modal === 'settings' && (
        <SettingsModal onClose={() => setModal('none')} />
      )}
```

- [ ] **Step 8: Typecheck + run full frontend suite**

```bash
cd codex-rs/lemurclaw-gui/assets && npx tsc --noEmit && npm test
```
Expected: `tsc` clean; all tests pass.

- [ ] **Step 9: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/settings/ \
        codex-rs/lemurclaw-gui/assets/src/components/TopBar.tsx \
        codex-rs/lemurclaw-gui/assets/src/app/App.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): add <SettingsModal> shell + gear icon in TopBar

Wide modal with a left-nav of 10 settings surfaces (permissions,
keymap, memories, skills, hooks, mcp, apps, plugins, experimental,
statusline) and a placeholder right pane. Surfaces are filled in
incrementally in Tasks 4-7. Adds ⚙ gear button to TopBar and a new
'settings' ModalKind in App.tsx."
```

---

## Task 4: Read-only list batch (permissions + hooks + mcp)

**Goal:** Replace the `Placeholder` for three surfaces with real panels that fetch read-only lists from the app-server. All three use `<SettingsListPicker>` without `onActivate` (display-only). Each panel owns its `useEffect` + `LoadState`.

**RPCs + types:**
- permissions → `permissionProfile/list` → `PermissionProfileListResponse` (`data: PermissionProfileSummary[]`, `nextCursor`)
- hooks → `hooks/list` → `HooksListResponse` (`data: HooksListEntry[]`, no cursor — flat list)
- mcp → `mcpServerStatus/list` → `McpServerStatusListResponse` (verify shape; if the file is absent, fall back to `{ data: McpServerStatus[] }`)

**Files:**
- Create: `assets/src/components/settings/PermissionsPanel.tsx`
- Create: `assets/src/components/settings/HooksPanel.tsx`
- Create: `assets/src/components/settings/McpPanel.tsx`
- Create: `assets/src/components/settings/__tests__/PermissionsPanel.test.tsx`
- Create: `assets/src/components/settings/__tests__/HooksPanel.test.tsx`
- Create: `assets/src/components/settings/__tests__/McpPanel.test.tsx`
- Modify: `assets/src/components/settings/SettingsModal.tsx`

- [ ] **Step 1: Verify the MCP list response shape**

Run:
```bash
cat codex-rs/lemurclaw-gui/assets/src/types/v2/McpServerStatusListResponse.ts 2>/dev/null \
  || ls codex-rs/lemurclaw-gui/assets/src/types/v2/ | grep -i mcpserverstatuslist
```
If `McpServerStatusListResponse.ts` exists, use it. If not, type the panel against `{ data: McpServerStatus[] }` directly.

- [ ] **Step 2: Write failing tests for all three panels**

Create `assets/src/components/settings/__tests__/PermissionsPanel.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { PermissionsPanel } from '../PermissionsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('PermissionsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists permission profiles and marks disallowed ones disabled', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { id: 'default', description: 'Default', allowed: true },
        { id: 'restricted', description: 'Restricted', allowed: false },
      ],
      nextCursor: null,
    });
    render(<PermissionsPanel />);
    await waitFor(() => expect(screen.getByText('Default')).toBeInTheDocument());
    expect(screen.getByText('restricted').closest('.settings-list-item')).toHaveClass('settings-list-item-disabled');
  });

  it('shows error state', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('boom'));
    render(<PermissionsPanel />);
    await waitFor(() => expect(screen.getByText(/boom/)).toBeInTheDocument());
  });
});
```

Create `assets/src/components/settings/__tests__/HooksPanel.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { HooksPanel } from '../HooksPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('HooksPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists hook entries grouped by cwd', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { cwd: '/repo', hooks: [{ event: 'PreToolUse', command: 'echo hi' } as never], warnings: [], errors: [] },
      ],
    });
    render(<HooksPanel />);
    await waitFor(() => expect(screen.getByText('/repo')).toBeInTheDocument());
    expect(screen.getByText(/PreToolUse/)).toBeInTheDocument();
  });

  it('shows empty state when no hooks configured', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [] });
    render(<HooksPanel />);
    await waitFor(() => expect(screen.getByText(/no hooks configured/i)).toBeInTheDocument());
  });
});
```

Create `assets/src/components/settings/__tests__/McpPanel.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { McpPanel } from '../McpPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('McpPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists MCP servers with tool count', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { name: 'fs', serverInfo: { name: 'fs', version: '1.0' }, tools: { a: {}, b: {} }, resources: [], resourceTemplates: [], authStatus: 'ok' as never },
      ],
    });
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText('fs')).toBeInTheDocument());
    expect(screen.getByText(/2 tools/)).toBeInTheDocument();
  });

  it('shows empty state', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [] });
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText(/no mcp servers/i)).toBeInTheDocument());
  });
});
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/__tests__/`
Expected: FAIL — modules not found.

- [ ] **Step 4: Implement `PermissionsPanel`**

Create `assets/src/components/settings/PermissionsPanel.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { PermissionProfileListResponse } from '../../types/v2/PermissionProfileListResponse';
import type { PermissionProfileSummary } from '../../types/v2/PermissionProfileSummary';

/** Read-only list of permission profiles from `permissionProfile/list`.
 *  Profiles with `allowed: false` are shown disabled (the effective
 *  requirements forbid selecting them). No activation — switching the active
 *  profile is a separate RPC not in this batch. */
export function PermissionsPanel() {
  const [state, setState] = useState<LoadState<PermissionProfileSummary>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<PermissionProfileListResponse>('permissionProfile/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, items: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(p) => p.id}
      renderLabel={(p) => p.id}
      renderSub={(p) => p.description}
      isDisabled={(p) => !p.allowed}
      emptyText="(no permission profiles configured)"
    />
  );
}
```

- [ ] **Step 5: Implement `HooksPanel`**

Create `assets/src/components/settings/HooksPanel.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { HooksListResponse } from '../../types/v2/HooksListResponse';
import type { HooksListEntry } from '../../types/v2/HooksListEntry';

/** Read-only list of configured hooks grouped by cwd (`hooks/list`). Each row
 *  is one cwd's entry; the sub-line summarizes hook count and any warnings.
 *  Read-only in this batch — adding/removing hooks is out of scope. */
export function HooksPanel() {
  const [state, setState] = useState<LoadState<HooksListEntry>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<HooksListResponse>('hooks/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, items: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(e) => e.cwd}
      renderLabel={(e) => e.cwd}
      renderSub={(e) => {
        const parts: string[] = [`${e.hooks.length} hook${e.hooks.length === 1 ? '' : 's'}`];
        if (e.warnings.length > 0) parts.push(`${e.warnings.length} warning${e.warnings.length === 1 ? '' : 's'}`);
        if (e.errors.length > 0) parts.push(`${e.errors.length} error${e.errors.length === 1 ? '' : 's'}`);
        return parts.join(' · ');
      }}
      emptyText="(no hooks configured)"
    />
  );
}
```

- [ ] **Step 6: Implement `McpPanel`**

Create `assets/src/components/settings/McpPanel.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { McpServerStatus } from '../../types/v2/McpServerStatus';

interface McpListResponse { data: McpServerStatus[] }

/** Read-only list of MCP servers from `mcpServerStatus/list`. Shows the server
 *  name + tool count. Read-only in this batch — server enable/disable is out
 *  of scope. */
export function McpPanel() {
  const [state, setState] = useState<LoadState<McpServerStatus>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<McpListResponse>('mcpServerStatus/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, items: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(s) => s.name}
      renderLabel={(s) => s.name}
      renderSub={(s) => `${Object.keys(s.tools).length} tools`}
      emptyText="(no MCP servers configured)"
    />
  );
}
```

- [ ] **Step 7: Wire the three panels into `<SettingsModal>`**

Edit `assets/src/components/settings/SettingsModal.tsx`. Add imports + conditional renders:

```tsx
import { PermissionsPanel } from './PermissionsPanel';
import { HooksPanel } from './HooksPanel';
import { McpPanel } from './McpPanel';
```

Replace the right-pane body:
```tsx
        <div className="settings-pane" data-testid={`settings-pane-${surface}`}>
          {surface === 'permissions' && <PermissionsPanel />}
          {surface === 'hooks' && <HooksPanel />}
          {surface === 'mcp' && <McpPanel />}
          {(surface === 'keymap' || surface === 'memories' || surface === 'skills'
            || surface === 'apps' || surface === 'plugins' || surface === 'experimental'
            || surface === 'statusline') && <Placeholder surface={surface} />}
        </div>
```

- [ ] **Step 8: Run the panel tests**

```bash
cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/
```
Expected: all panel tests + SettingsModal shell tests pass.

- [ ] **Step 9: Typecheck + full suite**

```bash
cd codex-rs/lemurclaw-gui/assets && npx tsc --noEmit && npm test
```
Expected: clean; all tests pass.

- [ ] **Step 10: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/settings/
git commit -m "feat(gui): settings read-only list panels (permissions/hooks/mcp)

Three panels built on <SettingsListPicker>:
- PermissionsPanel: permissionProfile/list, disabled rows for
  disallowed profiles.
- HooksPanel: hooks/list, one row per cwd with hook/warning/error
  counts.
- McpPanel: mcpServerStatus/list, one row per server with tool count.

All read-only (no activation). Wired into <SettingsModal>."
```

---

## Task 5: Editable list batch (skills + apps + plugins)

**Goal:** Three panels where each row carries a trailing action. These compose `<SettingsListPicker>` with `renderAction` for the per-row button.

**RPCs + types:**
- skills → `skills/list` (read only in this batch; `skills/config/write` + `skills/extraRoots/set` deferred)
- apps → `app/list` (read only; install/uninstall deferred)
- plugins → `plugin/list` (read) + `plugin/uninstall` (write)

**Scope guard:** Skills and Apps are display-only in this batch. Plugins gets a real uninstall action. This keeps the batch well under the 800-line change guidance.

**Files:**
- Create: `assets/src/components/settings/SkillsPanel.tsx`
- Create: `assets/src/components/settings/AppsPanel.tsx`
- Create: `assets/src/components/settings/PluginsPanel.tsx`
- Create: `assets/src/components/settings/__tests__/SkillsPanel.test.tsx`
- Create: `assets/src/components/settings/__tests__/AppsPanel.test.tsx`
- Create: `assets/src/components/settings/__tests__/PluginsPanel.test.tsx`
- Modify: `assets/src/components/settings/SettingsModal.tsx`

- [ ] **Step 1: Inspect the actual list-response shapes**

Run and read:
```bash
cat codex-rs/lemurclaw-gui/assets/src/types/v2/SkillsListEntry.ts \
    codex-rs/lemurclaw-gui/assets/src/types/v2/SkillMetadata.ts \
    codex-rs/lemurclaw-gui/assets/src/types/v2/AppsListResponse.ts \
    codex-rs/lemurclaw-gui/assets/src/types/v2/AppSummary.ts \
    codex-rs/lemurclaw-gui/assets/src/types/v2/PluginListResponse.ts \
    codex-rs/lemurclaw-gui/assets/src/types/v2/PluginMarketplaceEntry.ts \
    codex-rs/lemurclaw-gui/assets/src/types/v2/PluginUninstallParams.ts 2>/dev/null
```
Adjust the panel impl below to match the real field names. The test fixtures use placeholder fields — replace with the real ones from these types before running tests.

- [ ] **Step 2: Write failing tests (adjust fixtures to real types from Step 1)**

Create `assets/src/components/settings/__tests__/SkillsPanel.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { SkillsPanel } from '../SkillsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('SkillsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists skills from skills/list', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [{ cwd: '/repo', skills: [{ name: 'pdf', description: 'PDF skill' } as never], errors: [] }],
    });
    render(<SkillsPanel />);
    await waitFor(() => expect(screen.getByText('pdf')).toBeInTheDocument());
  });

  it('shows empty state when no skills', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [] });
    render(<SkillsPanel />);
    await waitFor(() => expect(screen.getByText(/no skills/i)).toBeInTheDocument());
  });
});
```

Create `assets/src/components/settings/__tests__/AppsPanel.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { AppsPanel } from '../AppsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('AppsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists apps from app/list', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [{ name: 'my-app', summary: 'An app' } as never],
      nextCursor: null,
    });
    render(<AppsPanel />);
    await waitFor(() => expect(screen.getByText('my-app')).toBeInTheDocument());
  });
});
```

Create `assets/src/components/settings/__tests__/PluginsPanel.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { PluginsPanel } from '../PluginsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('PluginsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists plugins and offers uninstall on installed ones', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [{ name: 'my-plugin', installed: true } as never],
      nextCursor: null,
    });
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('my-plugin')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /uninstall/i })).toBeInTheDocument();
  });

  it('uninstall button calls plugin/uninstall and refetches', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({ data: [{ name: 'p', installed: true } as never], nextCursor: null })
      .mockResolvedValueOnce({}) // plugin/uninstall response
      .mockResolvedValueOnce({ data: [], nextCursor: null }); // refetch
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('p')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /uninstall/i }));
    await waitFor(() => expect(screen.queryByText('p')).not.toBeInTheDocument());
  });
});
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/__tests__/`
Expected: FAIL — modules not found.

- [ ] **Step 4: Implement `SkillsPanel`** (read-only display in this batch)

Create `assets/src/components/settings/SkillsPanel.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { SkillsListResponse } from '../../types/v2/SkillsListResponse';
import type { SkillMetadata } from '../../types/v2/SkillMetadata';

/** Read-only list of discovered skills (`skills/list`), flattened across cwds.
 *  Each row = one skill; sub-line shows the description (if any) and the cwd
 *  it was discovered under. Editing skills is out of scope for this batch. */
interface SkillRow { name: string; description: string | null; cwd: string }

export function SkillsPanel() {
  const [state, setState] = useState<LoadState<SkillRow>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<SkillsListResponse>('skills/list', {})
      .then((resp) => {
        if (cancelled) return;
        const rows: SkillRow[] = resp.data.flatMap((entry) =>
          (entry.skills ?? []).map((s: SkillMetadata) => ({
            name: s.name,
            description: ('description' in s && typeof s.description === 'string') ? s.description : null,
            cwd: entry.cwd,
          })),
        );
        setState({ loading: false, error: null, items: rows });
      })
      .catch((e) => {
        if (cancelled) return;
        setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(r) => `${r.cwd}::${r.name}`}
      renderLabel={(r) => r.name}
      renderSub={(r) => r.description ?? r.cwd}
      emptyText="(no skills discovered)"
    />
  );
}
```

If `SkillMetadata` has no `name` field, use the actual identifier field from Step 1 and document the deviation in the commit message.

- [ ] **Step 5: Implement `AppsPanel`** (read-only display)

Create `assets/src/components/settings/AppsPanel.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { AppsListResponse } from '../../types/v2/AppsListResponse';
import type { AppSummary } from '../../types/v2/AppSummary';

/** Read-only list of registered apps (`app/list`). Install/uninstall is out
 *  of scope for this batch. */
export function AppsPanel() {
  const [state, setState] = useState<LoadState<AppSummary>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<AppsListResponse>('app/list', {})
      .then((resp) => {
        if (cancelled) setState({ loading: false, error: null, items: resp.data });
      })
      .catch((e) => {
        if (cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(a) => ('name' in a && a.name) ? String(a.name) : JSON.stringify(a)}
      renderLabel={(a) => 'name' in a && a.name ? String(a.name) : '(unnamed app)'}
      renderSub={(a) => 'summary' in a && typeof a.summary === 'string' ? a.summary : undefined}
      emptyText="(no apps registered)"
    />
  );
}
```

Adjust the `getId`/`renderLabel`/`renderSub` field accessors to the real `AppSummary` shape from Step 1.

- [ ] **Step 6: Implement `PluginsPanel`** (with uninstall action)

Create `assets/src/components/settings/PluginsPanel.tsx`:

```tsx
import { useCallback, useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { PluginListResponse } from '../../types/v2/PluginListResponse';
import type { PluginMarketplaceEntry } from '../../types/v2/PluginMarketplaceEntry';

/** Plugin list (`plugin/list`) with an `uninstall` action per installed plugin.
 *  Uninstall calls `plugin/uninstall` then refetches the list. Install-from-
 *  marketplace is out of scope for this batch. */
export function PluginsPanel() {
  const [state, setState] = useState<LoadState<PluginMarketplaceEntry>>({
    loading: true, error: null, items: [],
  });

  const fetchAll = useCallback(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    return sendRequest<PluginListResponse>('plugin/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, items: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      })
      .finally(() => { cancelled = true; });
  }, []);

  useEffect(() => { fetchAll(); }, [fetchAll]);

  const uninstall = async (name: string) => {
    await sendRequest('plugin/uninstall', { name });
    fetchAll();
  };

  return (
    <SettingsListPicker
      state={state}
      getId={(p) => p.name}
      renderLabel={(p) => p.name}
      renderSub={(p) => ('installed' in p && p.installed) ? 'installed' : 'available'}
      renderAction={(p) =>
        ('installed' in p && p.installed) ? (
          <button className="settings-action-button" onClick={() => uninstall(p.name)}>uninstall</button>
        ) : null
      }
      emptyText="(no plugins in marketplace)"
    />
  );
}
```

If `PluginMarketplaceEntry` doesn't have `name`/`installed` fields, use the real identifiers from Step 1.

- [ ] **Step 7: Add `.settings-action-button` style**

Append to `assets/src/styles.css`:

```css
.settings-action-button { background: none; border: 1px solid var(--border); border-radius: 4px; padding: 3px 8px; font-size: 11px; cursor: pointer; color: var(--muted); }
.settings-action-button:hover { color: inherit; border-color: var(--muted); }
```

- [ ] **Step 8: Wire the three panels into `<SettingsModal>`**

Edit `assets/src/components/settings/SettingsModal.tsx` to import + render `SkillsPanel`, `AppsPanel`, `PluginsPanel`. The `Placeholder` now only covers `keymap`, `memories`, `experimental`, `statusline`.

- [ ] **Step 9: Run panel tests + typecheck + full suite**

```bash
cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/ && npx tsc --noEmit && npm test
```
Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/settings/ codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): settings editable list panels (skills/apps/plugins)

- SkillsPanel: skills/list, flattened across cwds, read-only display.
- AppsPanel: app/list, read-only display.
- PluginsPanel: plugin/list with per-row uninstall action that calls
  plugin/uninstall and refetches.

All three compose <SettingsListPicker> with renderAction. Wired into
<SettingsModal>."
```

---

## Task 6: Toggle surface — experimental features

**Goal:** A panel listing experimental features from `experimentalFeature/list`, each with a toggle that calls `experimentalFeature/enablement/set`. Updates local state optimistically and reverts on failure.

**Files:**
- Create: `assets/src/components/settings/ExperimentalPanel.tsx`
- Create: `assets/src/components/settings/__tests__/ExperimentalPanel.test.tsx`
- Modify: `assets/src/components/settings/SettingsModal.tsx`

- [ ] **Step 1: Write failing test**

Create `assets/src/components/settings/__tests__/ExperimentalPanel.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { ExperimentalPanel } from '../ExperimentalPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('ExperimentalPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists features with toggle state matching enabled', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { name: 'feat-a', stage: 'beta', displayName: 'Feat A', description: 'desc', announcement: null, enabled: true, defaultEnabled: false },
        { name: 'feat-b', stage: 'alpha', displayName: null, description: null, announcement: null, enabled: false, defaultEnabled: false },
      ],
      nextCursor: null,
    });
    render(<ExperimentalPanel />);
    await waitFor(() => expect(screen.getByText('Feat A')).toBeInTheDocument());
    expect(screen.getByTestId('toggle-feat-a')).toBeChecked();
    expect(screen.getByTestId('toggle-feat-b')).not.toBeChecked();
  });

  it('toggling fires experimentalFeature/enablement/set', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({
        data: [{ name: 'feat-a', stage: 'beta', displayName: 'Feat A', description: null, announcement: null, enabled: false, defaultEnabled: false }],
        nextCursor: null,
      })
      .mockResolvedValueOnce({}); // enablement/set ack
    render(<ExperimentalPanel />);
    await waitFor(() => expect(screen.getByText('Feat A')).toBeInTheDocument());
    fireEvent.click(screen.getByTestId('toggle-feat-a'));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('experimentalFeature/enablement/set', { enablement: { 'feat-a': true } });
    });
  });

  it('shows error state', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('boom'));
    render(<ExperimentalPanel />);
    await waitFor(() => expect(screen.getByText(/boom/)).toBeInTheDocument());
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/__tests__/ExperimentalPanel.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `ExperimentalPanel`**

Create `assets/src/components/settings/ExperimentalPanel.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import type { ExperimentalFeatureListResponse } from '../../types/v2/ExperimentalFeatureListResponse';
import type { ExperimentalFeature } from '../../types/v2/ExperimentalFeature';

interface LoadState { loading: boolean; error: string | null; features: ExperimentalFeature[] }

/** Experimental feature toggle panel. Loads `experimentalFeature/list` once;
 *  each row has a checkbox bound to its `enabled` flag. Toggling fires
 *  `experimentalFeature/enablement/set` with the single delta and updates local
 *  state optimistically; reverts + surfaces the error on failure (the response
 *  is ack-only, so no refetch). */
export function ExperimentalPanel() {
  const [state, setState] = useState<LoadState>({ loading: true, error: null, features: [] });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, features: [] });
    sendRequest<ExperimentalFeatureListResponse>('experimentalFeature/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, features: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), features: [] });
      });
    return () => { cancelled = true; };
  }, []);

  const toggle = (feat: ExperimentalFeature, next: boolean) => {
    setState((s) => ({
      ...s,
      features: s.features.map((f) => f.name === feat.name ? { ...f, enabled: next } : f),
    }));
    sendRequest('experimentalFeature/enablement/set', { enablement: { [feat.name]: next } }).catch((e) => {
      setState((s) => ({
        ...s,
        error: e instanceof Error ? e.message : String(e),
        features: s.features.map((f) => f.name === feat.name ? { ...f, enabled: !next } : f),
      }));
    });
  };

  if (state.loading) return <div className="modal-loading">loading…</div>;
  if (state.error) return <div className="modal-error">failed: {state.error}</div>;
  if (state.features.length === 0) return <div className="modal-empty">(no experimental features)</div>;

  return (
    <ul className="settings-list">
      {state.features.map((f) => (
        <li key={f.name} className="settings-list-item">
          <label className="experimental-feature-row">
            <input
              type="checkbox"
              data-testid={`toggle-${f.name}`}
              checked={f.enabled}
              onChange={(e) => toggle(f, e.target.checked)}
            />
            <span className="settings-list-item-label">
              {f.displayName ?? f.name}
              {f.stage !== 'beta' && <span className="experimental-feature-stage"> · {f.stage}</span>}
            </span>
            {f.description && <span className="settings-list-item-sub">{f.description}</span>}
          </label>
        </li>
      ))}
    </ul>
  );
}
```

- [ ] **Step 4: Add experimental-feature styles**

Append to `assets/src/styles.css`:

```css
.experimental-feature-row { display: flex; align-items: center; gap: 8px; padding: 8px 12px; cursor: pointer; flex: 1; }
.experimental-feature-stage { color: var(--muted); font-size: 11px; }
```

- [ ] **Step 5: Wire into `<SettingsModal>`**

Edit `assets/src/components/settings/SettingsModal.tsx` to import `ExperimentalPanel` and add `surface === 'experimental' && <ExperimentalPanel />`. Placeholder now only covers `keymap`, `memories`, `statusline`.

- [ ] **Step 6: Run tests + typecheck + full suite**

```bash
cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/ && npx tsc --noEmit && npm test
```
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/settings/ codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): settings experimental features panel

Lists experimentalFeature/list with a per-row checkbox bound to enabled.
Toggling fires experimentalFeature/enablement/set with the single delta
and updates local state optimistically; reverts + surfaces error on
failure. Wired into <SettingsModal>."
```

---

## Task 7 (revised): Config-form surfaces (memories + model settings)

**Goal:** Two surfaces that read/write real config keys via `config/read` + `config/value/write`. A generic `<SettingsForm>` renders a labeled textarea bound to a single value, with save/revert buttons. Thin wrapper panels supply the surface-specific key + label.

**Scope revision (post-discovery):** The original plan had keymap + statusline + terminal-title as config-form surfaces, but verification against the app-server protocol (`codex-rs/app-server-protocol/src/protocol/v2/config.rs`) showed the wire `Config` struct has NO `tui` field — those keys live under codex's internal `[tui]` config section which is never serialized over the app-server API. The `desktop` HashMap field exists but codex doesn't read `[desktop].statusline` etc., so editing those keys would be inert. The three TUI-only surfaces are dropped. Replaced with a `model` panel editing real `Config` fields.

**Config key paths (verified):**
- memories: `developer_instructions` (typed `Option<String>` on `Config` — confirmed)
- model: `model`, `model_provider`, `model_reasoning_effort`, `model_reasoning_summary`, `model_verbosity` (all typed fields on `Config`)
- `MergeStrategy` enum values: `"replace" | "upsert"` (verified in `MergeStrategy.ts`)

**Files:**
- Create: `assets/src/components/settings/SettingsForm.tsx` (generic)
- Create: `assets/src/components/settings/MemoriesPanel.tsx`
- Create: `assets/src/components/settings/ModelPanel.tsx`
- Create: `assets/src/components/settings/__tests__/SettingsForm.test.tsx`
- Create: `assets/src/components/settings/__tests__/MemoriesPanel.test.tsx`
- Create: `assets/src/components/settings/__tests__/ModelPanel.test.tsx`
- Modify: `assets/src/components/settings/SettingsModal.tsx`

- [ ] **Step 1: Write failing test for `<SettingsForm>`**

Create `assets/src/components/settings/__tests__/SettingsForm.test.tsx`:

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { SettingsForm } from '../SettingsForm';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('SettingsForm', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('loads the value via config/read and shows it in the textarea', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      config: { developerInstructions: 'remember to use rust 2021 edition' },
      origins: {},
      layers: null,
    });
    render(<SettingsForm configKey="developer_instructions" label="Memories" />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toHaveValue('remember to use rust 2021 edition'));
  });

  it('save button fires config/value/write with the new value', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({ config: { developerInstructions: 'old' }, origins: {}, layers: null })
      .mockResolvedValueOnce({}); // write ack
    render(<SettingsForm configKey="developer_instructions" label="Memories" />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toHaveValue('old'));
    fireEvent.change(screen.getByLabelText('Memories'), { target: { value: 'new' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('config/value/write', expect.objectContaining({
        keyPath: 'developer_instructions',
        value: 'new',
        mergeStrategy: 'replace',
      }));
    });
  });

  it('revert button restores the loaded value', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      config: { developerInstructions: 'loaded' }, origins: {}, layers: null,
    });
    render(<SettingsForm configKey="developer_instructions" label="Memories" />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toHaveValue('loaded'));
    fireEvent.change(screen.getByLabelText('Memories'), { target: { value: 'edited' } });
    fireEvent.click(screen.getByRole('button', { name: /revert/i }));
    expect(screen.getByLabelText('Memories')).toHaveValue('loaded');
  });

  it('shows error state on failed load', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('nope'));
    render(<SettingsForm configKey="developer_instructions" label="Memories" />);
    await waitFor(() => expect(screen.getByText(/nope/)).toBeInTheDocument());
  });

  it('disables save/revert when not dirty', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      config: { developerInstructions: 'loaded' }, origins: {}, layers: null,
    });
    render(<SettingsForm configKey="developer_instructions" label="Memories" />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toHaveValue('loaded'));
    expect(screen.getByRole('button', { name: /save/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /revert/i })).toBeDisabled();
  });
});
```

**Note on field name:** the wire `Config` type uses camelCase on the wire (per app-server v2 conventions in AGENTS.md), so `developer_instructions` arrives as `developerInstructions` in the JSON response. The `<SettingsForm>` reads `config[configKey]` where `configKey` is the camelCase wire name. The `config/value/write` request uses the snake_case TOML key (`keyPath: 'developer_instructions'`) — that's the config-file path, not the wire field name. Both are correct: read = wire field name, write = TOML key path.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/__tests__/SettingsForm.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `<SettingsForm>`**

Create `assets/src/components/settings/SettingsForm.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import type { ConfigReadResponse } from '../../types/v2/ConfigReadResponse';

interface Props {
  /** camelCase wire field name on the Config object (e.g. "developerInstructions").
   *  Used to read the current value out of config/read's response. */
  configKey: string;
  /** snake_case TOML key path for config/value/write (e.g. "developer_instructions").
   *  Defaults to the same string as configKey when the wire name matches the TOML key. */
  writeKeyPath?: string;
  /** Visible label above the textarea. */
  label: string;
  /** Optional helper text below the label. */
  hint?: string;
}

/** Generic single-value config editor. Reads the wire `configKey` from
 *  `config/read`, shows the value as a string in a textarea, and writes back
 *  via `config/value/write` (with `writeKeyPath` as the TOML key path, defaulting
 *  to configKey). Revert restores the last-loaded value.
 *
 *  Values are treated as opaque strings — no schema validation here; callers
 *  that care can wrap with their own validator. */
function getField(obj: unknown, key: string): unknown {
  if (obj && typeof obj === 'object' && key in (obj as Record<string, unknown>)) {
    return (obj as Record<string, unknown>)[key];
  }
  return undefined;
}

export function SettingsForm({ configKey, writeKeyPath, label, hint }: Props) {
  const path = writeKeyPath ?? configKey;
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState<string>('');
  const [draft, setDraft] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    sendRequest<ConfigReadResponse>('config/read', {})
      .then((resp) => {
        if (cancelled) return;
        const v = getField(resp.config, configKey);
        const s = v === undefined || v === null ? '' : typeof v === 'string' ? v : JSON.stringify(v);
        setLoaded(s);
        setDraft(s);
        setLoading(false);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
        setLoading(false);
      });
    return () => { cancelled = true; };
  }, [configKey]);

  const save = async () => {
    try {
      await sendRequest('config/value/write', { keyPath: path, value: draft, mergeStrategy: 'replace' });
      setLoaded(draft);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  if (loading) return <div className="modal-loading">loading…</div>;
  if (error) return <div className="modal-error">failed: {error}</div>;

  const dirty = draft !== loaded;
  return (
    <div className="settings-form">
      <label className="settings-form-label" htmlFor={`sf-${configKey}`}>{label}</label>
      {hint && <div className="settings-form-hint">{hint}</div>}
      <textarea
        id={`sf-${configKey}`}
        className="settings-form-textarea"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        rows={8}
      />
      <div className="settings-form-actions">
        <button className="settings-action-button" onClick={save} disabled={!dirty}>save</button>
        <button className="settings-action-button" onClick={() => setDraft(loaded)} disabled={!dirty}>revert</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run `<SettingsForm>` test to verify it passes**

Run: `cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/__tests__/SettingsForm.test.tsx`
Expected: PASS (5 tests).

- [ ] **Step 5: Write test for `MemoriesPanel` (representative wrapper)**

Create `assets/src/components/settings/__tests__/MemoriesPanel.test.tsx`:

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoriesPanel } from '../MemoriesPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('MemoriesPanel', () => {
  it('renders a SettingsForm labeled "Memories"', () => {
    vi.mocked(sendRequest).mockResolvedValue({ config: {}, origins: {}, layers: null });
    render(<MemoriesPanel />);
    expect(screen.getByLabelText('Memories')).toBeInTheDocument();
  });
});
```

- [ ] **Step 6: Write test for `ModelPanel`**

Create `assets/src/components/settings/__tests__/ModelPanel.test.tsx`:

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ModelPanel } from '../ModelPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('ModelPanel', () => {
  it('renders SettingsForms for model, provider, reasoning effort, verbosity', () => {
    vi.mocked(sendRequest).mockResolvedValue({ config: {}, origins: {}, layers: null });
    render(<ModelPanel />);
    expect(screen.getByLabelText('Model id')).toBeInTheDocument();
    expect(screen.getByLabelText('Model provider')).toBeInTheDocument();
    expect(screen.getByLabelText('Reasoning effort')).toBeInTheDocument();
    expect(screen.getByLabelText('Verbosity')).toBeInTheDocument();
  });
});
```

- [ ] **Step 7: Implement the two wrapper panels**

Create `assets/src/components/settings/MemoriesPanel.tsx`:

```tsx
import { SettingsForm } from './SettingsForm';

/** Memories editor. Backed by `developer_instructions` (a typed Option<String>
 *  field on the wire Config). This is the free-form text the agent sees as
 *  persistent memory across turns. */
export function MemoriesPanel() {
  return (
    <SettingsForm
      configKey="developerInstructions"
      writeKeyPath="developer_instructions"
      label="Memories"
      hint="Free-form text shown to the agent as persistent memory."
    />
  );
}
```

Create `assets/src/components/settings/ModelPanel.tsx`:

```tsx
import { SettingsForm } from './SettingsForm';

/** Model settings editor. Exposes the model-related scalar fields on Config
 *  that the app-server actually serializes: model id, provider, reasoning
 *  effort, and verbosity. Each is its own <SettingsForm> row. */
export function ModelPanel() {
  return (
    <div className="settings-form-stack">
      <SettingsForm configKey="model" label="Model id" hint="e.g. gpt-5.2" />
      <SettingsForm configKey="modelProvider" writeKeyPath="model_provider" label="Model provider" hint="e.g. openai, openai-compatible" />
      <SettingsForm configKey="modelReasoningEffort" writeKeyPath="model_reasoning_effort" label="Reasoning effort" hint="minimal | low | medium | high" />
      <SettingsForm configKey="modelVerbosity" writeKeyPath="model_verbosity" label="Verbosity" hint="default | verbose" />
    </div>
  );
}
```

- [ ] **Step 8: Add form styles**

Append to `assets/src/styles.css`:

```css
.settings-form { display: flex; flex-direction: column; gap: 8px; }
.settings-form-label { font-size: 13px; font-weight: 600; }
.settings-form-hint { font-size: 11px; color: var(--muted); }
.settings-form-textarea { width: 100%; font-family: var(--mono, monospace); font-size: 12px; padding: 8px; background: var(--cell-bg); color: inherit; border: 1px solid var(--border); border-radius: 4px; resize: vertical; }
.settings-form-actions { display: flex; gap: 8px; }
.settings-form-actions .settings-action-button:disabled { opacity: 0.4; cursor: default; }
.settings-form-stack { display: flex; flex-direction: column; gap: 16px; }
```

- [ ] **Step 9: Wire into `<SettingsModal>` — Placeholder is gone**

Edit `assets/src/components/settings/SettingsModal.tsx` to import + render `MemoriesPanel` and `ModelPanel` for the matching surfaces. **Delete the `Placeholder` component** — every surface now has a real panel.

The right-pane body becomes:
```tsx
        <div className="settings-pane" data-testid={`settings-pane-${surface}`}>
          {surface === 'permissions' && <PermissionsPanel />}
          {surface === 'memories' && <MemoriesPanel />}
          {surface === 'model' && <ModelPanel />}
          {surface === 'skills' && <SkillsPanel />}
          {surface === 'hooks' && <HooksPanel />}
          {surface === 'mcp' && <McpPanel />}
          {surface === 'apps' && <AppsPanel />}
          {surface === 'plugins' && <PluginsPanel />}
          {surface === 'experimental' && <ExperimentalPanel />}
        </div>
```

- [ ] **Step 10: Run tests + typecheck + full suite**

```bash
cd codex-rs/lemurclaw-gui/assets && npx vitest run src/components/settings/ && npx tsc --noEmit && npm test
```
Expected: all pass. No remaining "coming soon" placeholders.

- [ ] **Step 11: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/settings/ codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): settings config-form surfaces (memories + model)

- SettingsForm: generic config editor backed by config/read +
  config/value/write. Reads camelCase wire field, writes snake_case
  TOML key path. Textarea + save/revert, dirty-aware buttons.
- MemoriesPanel: developer_instructions.
- ModelPanel: model + model_provider + reasoning_effort + verbosity
  (stacked forms).

Scope note: keymap/statusline/terminal-title are dropped — they live
under codex's [tui] config which the app-server config/read API does
not expose. All Placeholder cases removed from <SettingsModal>."
```

---


## Task 8: Final review + branch finishing

**Goal:** Confirm the whole subproject hangs together, no dead code, no Placeholder remnants, full test suite green, typecheck clean. Then run the finishing-a-development-branch skill to decide merge/PR/keep-branch.

**Files:** No new files; this task is verification + the finishing skill.

- [ ] **Step 1: Verify no Placeholder remnants**

```bash
grep -rn "coming soon\|Placeholder" codex-rs/lemurclaw-gui/assets/src/components/settings/
```
Expected: no matches.

- [ ] **Step 2: Full frontend test suite + typecheck**

```bash
cd codex-rs/lemurclaw-gui/assets && npm test && npx tsc --noEmit
```
Expected: all tests pass; typecheck clean.

- [ ] **Step 3: Smoke-test the Rust GUI build (no behavior change expected)**

From repo root:
```bash
cargo check -p lemurclaw-gui
```
Expected: compiles (the GUI frontend is embedded via include_dir / build script — this catches asset path regressions).

- [ ] **Step 4: Review the commit log + diff stats**

```bash
git log --oneline main..HEAD
git diff --stat main..HEAD
```
Expected: 7 commits (Tasks 1-7); total diff well under the 800-line-per-change guidance (each task is its own reviewable stage). If total changed lines exceed ~800 across the whole subproject, that is expected here because the changes are split into 7 independent reviewable stages — each stage individually is under 500 lines.

- [ ] **Step 5: Invoke the finishing-a-development-branch skill**

Use the `superpowers:finishing-a-development-branch` skill to decide: merge to main, open a PR, or keep the branch for further work. Default for this subproject (per the handoff): keep all changes on `main` as a sequence of commits (matches subprojects 1-4's pattern of committing directly to main), unless the user prefers a feature branch + PR.

- [ ] **Step 6: Update handoff (if session is being handed off)**

If this work is being paused before Task 8 completes, write a fresh handoff capsule noting:
- Which task was last completed
- Test suite state (green/red)
- Any outstanding deviations from the plan (e.g., real config key paths that differed from the plan's guesses, real type field names)
- The manual e2e step still pending: `cargo run -p lemurclaw -- --frontend gui` on a display-capable machine

---

## Self-Review Notes

**Spec coverage** (10 config surfaces from the handoff): permissions (Task 4 ✓), keymap (Task 7 ✓), memories (Task 7 ✓), skills (Task 5 ✓), hooks (Task 4 ✓), mcp (Task 4 ✓), apps (Task 5 ✓), plugins (Task 5 ✓), experimental (Task 6 ✓), statusline+title (Task 7 ✓). Gear icon in TopBar (Task 3 ✓). Shared Modal refactor (Task 1 ✓).

**Type-consistency audit:** `<Modal>` props `title`/`onClose`/`children`/`testId`/`wide` are used identically across Tasks 1, 3. `<SettingsListPicker>` props `state`/`getId`/`renderLabel`/`renderSub`/`renderAction`/`isDisabled`/`activeId`/`onActivate`/`emptyText` match between Task 2 (definition) and Tasks 4-5 (use sites). `LoadState<T>` exported from `SettingsListPicker.tsx` is imported by all panel files. `SettingsSurface` type (Task 3) is the source of truth for the nav list.

**Placeholders:** None. Every code step shows complete code; verification steps (Task 4 Step 1, Task 5 Step 1, Task 7 Step 1) call out that real type field names must be confirmed before the test fixtures are finalized — these are explicit "verify then adjust" instructions, not TODOs.

**Scope discipline:** Skills/Apps panels are intentionally read-only in Task 5 (the handoff's "list+edit" mapping is downgraded to display for these two to keep the batch under 800 lines). This is called out explicitly in Task 5's scope guard. Future subproject can add install/uninstall/skill-config actions on top of the existing panel shells.
