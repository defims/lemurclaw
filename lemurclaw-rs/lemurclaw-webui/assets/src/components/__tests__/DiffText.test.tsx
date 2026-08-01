import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DiffText, parseDiffBlocks } from '../DiffText';

// Sample unified diffs used across tests.
const SINGLE_FILE_ADD =
  `diff --git a/new.txt b/new.txt\n` +
  `new file mode 100644\n` +
  `index 0000000..abc\n` +
  `--- /dev/null\n` +
  `+++ b/new.txt\n` +
  `@@ -0,0 +1,2 @@\n` +
  `+hello\n` +
  `+world\n`;

const SINGLE_FILE_UPDATE =
  `diff --git a/code.rs b/code.rs\n` +
  `index abc..def 100644\n` +
  `--- a/code.rs\n` +
  `+++ b/code.rs\n` +
  `@@ -1,2 +1,2 @@\n` +
  ` fn main() {\n` +
  `-    println!("old");\n` +
  `+    println!("new");\n` +
  ` }\n`;

const MULTI_FILE = SINGLE_FILE_ADD + SINGLE_FILE_UPDATE.replace('diff --git', 'diff --git');

describe('parseDiffBlocks', () => {
  it('returns empty array for empty input', () => {
    expect(parseDiffBlocks('')).toEqual([]);
  });

  it('parses a single add-only file into one block with 2 add lines', () => {
    const blocks = parseDiffBlocks(SINGLE_FILE_ADD);
    expect(blocks.length).toBe(1);
    expect(blocks[0].path).toBe('new.txt');
    const adds = blocks[0].lines.filter((l) => l.kind === 'add');
    expect(adds.length).toBe(2);
  });

  it('classifies context / del / hunk / meta lines correctly', () => {
    const blocks = parseDiffBlocks(SINGLE_FILE_UPDATE);
    const kinds = blocks[0].lines.map((l) => l.kind);
    expect(kinds).toContain('meta');    // diff --git / index / --- / +++
    expect(kinds).toContain('hunk');    // @@ ... @@
    expect(kinds).toContain('ctx');     //  fn main() / }
    expect(kinds).toContain('del');     // -println old
    expect(kinds).toContain('add');     // +println new
  });

  it('parses multiple files into multiple blocks', () => {
    const blocks = parseDiffBlocks(MULTI_FILE);
    expect(blocks.length).toBe(2);
    expect(blocks[0].path).toBe('new.txt');
    expect(blocks[1].path).toBe('code.rs');
  });

  it('detects rust language from .rs extension', () => {
    const blocks = parseDiffBlocks(SINGLE_FILE_UPDATE);
    expect(blocks[0].lang).toBe('rust');
  });

  it('falls back to "none" for unknown extensions', () => {
    const diff =
      `diff --git a/data.xyz b/data.xyz\n+++ b/data.xyz\n@@ -0,0 +1 @@\n+content\n`;
    const blocks = parseDiffBlocks(diff);
    expect(blocks[0].lang).toBe('none');
  });
});

describe('<DiffText>', () => {
  it('renders empty placeholder for empty input', () => {
    render(<DiffText diff="" />);
    expect(screen.getByText('(empty diff)')).toBeInTheDocument();
  });

  it('renders one block per file with path header', () => {
    render(<DiffText diff={SINGLE_FILE_ADD} />);
    expect(screen.getByText('new.txt')).toBeInTheDocument();
    expect(screen.getAllByTestId('diff-file-block').length).toBe(1);
  });

  it('renders multiple blocks for multi-file diff', () => {
    render(<DiffText diff={MULTI_FILE} />);
    expect(screen.getByText('new.txt')).toBeInTheDocument();
    expect(screen.getByText('code.rs')).toBeInTheDocument();
    expect(screen.getAllByTestId('diff-file-block').length).toBe(2);
  });

  it('emits per-line data-testid markers by kind', () => {
    render(<DiffText diff={SINGLE_FILE_UPDATE} />);
    expect(screen.getAllByTestId('diff-line-add').length).toBeGreaterThan(0);
    expect(screen.getAllByTestId('diff-line-del').length).toBeGreaterThan(0);
    expect(screen.getAllByTestId('diff-line-ctx').length).toBeGreaterThan(0);
    expect(screen.getAllByTestId('diff-line-hunk').length).toBeGreaterThan(0);
    expect(screen.getAllByTestId('diff-line-meta').length).toBeGreaterThan(0);
  });

  it('renders +N -M stats in each file header', () => {
    render(<DiffText diff={SINGLE_FILE_UPDATE} />);
    // 1 add, 1 del.
    expect(screen.getByText('+1')).toBeInTheDocument();
    expect(screen.getByText('-1')).toBeInTheDocument();
  });

  it('does not crash on unknown extension (falls back to plain render)', () => {
    const diff =
      `diff --git a/data.xyz b/data.xyz\n+++ b/data.xyz\n@@ -0,0 +1 @@\n+content\n`;
    render(<DiffText diff={diff} />);
    expect(screen.getByText('data.xyz')).toBeInTheDocument();
    expect(screen.getAllByTestId('diff-line-add').length).toBe(1);
  });
});
