import { Highlight, themes } from 'prism-react-renderer';
import type { Language } from 'prism-react-renderer';

/** Extension (lowercase, with dot) → prism language id. Unknown → 'none'.
 *  Keep small — only the languages codex/lemurclaw code is likely to touch. */
const LANG_BY_EXT: Record<string, Language> = {
  '.rs': 'rust',
  '.ts': 'typescript',
  '.tsx': 'tsx',
  '.js': 'javascript',
  '.jsx': 'jsx',
  '.py': 'python',
  '.md': 'markdown',
  '.css': 'css',
  '.html': 'markup',
  '.htm': 'markup',
  '.xml': 'markup',
  '.json': 'json',
  '.sh': 'bash',
  '.bash': 'bash',
  '.zsh': 'bash',
  '.toml': 'toml',
  '.yaml': 'yaml',
  '.yml': 'yaml',
  '.go': 'go',
  '.c': 'c',
  '.h': 'c',
  '.cpp': 'cpp',
  '.cc': 'cpp',
  '.hpp': 'cpp',
};

function detectLang(path: string): Language {
  const dot = path.lastIndexOf('.');
  if (dot < 0) return 'none';
  const ext = path.slice(dot).toLowerCase();
  return LANG_BY_EXT[ext] ?? 'none';
}

/** One classified line of a diff body. */
type DiffLineKind = 'add' | 'del' | 'ctx' | 'hunk' | 'meta';
interface DiffLine {
  kind: DiffLineKind;
  /** The line content WITHOUT the leading +/-/space prefix. For hunk/meta
   *  lines, the full original line. */
  content: string;
  /** The raw prefix character ('+', '-', ' ', or empty). Used by tests. */
  prefix: string;
}

/** One file's worth of a diff. */
interface DiffBlock {
  /** File path from the `b/...` side of `diff --git a/x b/x`. Falls back to
   *  the raw `+++ b/x` line if no `diff --git` header is present. */
  path: string;
  lang: Language;
  lines: DiffLine[];
}

/** Parse a unified diff string into per-file blocks.
 *
 *  Handles both `diff --git a/... b/...` style (git's default) and the simpler
 *  `--- /+++ ` style. If the input has neither (single-file inline diff),
 *  treats the whole input as one block with path '(unknown)'.
 *
 *  Lines are classified by first character:
 *    '+' (but not '+++') → add
 *    '-' (but not '---') → del
 *    ' '                  → ctx
 *    '@'                  → hunk header (@@ ... @@)
 *    anything else        → meta (diff --git, index, +++, ---, etc.)
 */
export function parseDiffBlocks(diff: string): DiffBlock[] {
  if (!diff) return [];
  const lines = diff.split('\n');
  const blocks: DiffBlock[] = [];
  let current: DiffBlock | null = null;

  for (const raw of lines) {
    const isDiffHeader = raw.startsWith('diff --git');
    const isPlusPlus = raw.startsWith('+++ ');
    const isMinusMinus = raw.startsWith('--- ');

    if (isDiffHeader) {
      // Start a new block. Try to extract path from "diff --git a/X b/Y".
      const m = raw.match(/^diff --git a\/(.*) b\/(.*)$/);
      const path = m ? m[2] : '(unknown)';
      current = { path, lang: detectLang(path), lines: [] };
      blocks.push(current);
      current.lines.push({ kind: 'meta', content: raw, prefix: '' });
      continue;
    }

    if (isPlusPlus) {
      // "+++ b/path" — file path for the new side.
      const m = raw.match(/^\+\+\+ (?:b\/)?(.+)$/);
      const path = m ? m[1] : '(unknown)';
      if (!current) {
        // No `diff --git` header seen — start a block now using +++ path.
        current = { path, lang: detectLang(path), lines: [] };
        blocks.push(current);
      } else if (current.lines.length === 0 || current.path === '(unknown)') {
        // Refine path/lang if we started the block blind.
        current.path = path;
        current.lang = detectLang(path);
      }
      current.lines.push({ kind: 'meta', content: raw, prefix: '' });
      continue;
    }

    if (isMinusMinus) {
      if (!current) {
        // Start block with --- path as placeholder; +++ will refine.
        const m = raw.match(/^--- (?:a\/)?(.+)$/);
        const path = m ? m[1] : '(unknown)';
        current = { path, lang: detectLang(path), lines: [] };
        blocks.push(current);
      }
      current.lines.push({ kind: 'meta', content: raw, prefix: '' });
      continue;
    }

    // Body line of an existing block (or unparseable orphan).
    if (!current) {
      // No header at all — wrap the whole input as a single block.
      current = { path: '(unknown)', lang: 'none', lines: [] };
      blocks.push(current);
    }

    let kind: DiffLineKind;
    let prefix: string;
    if (raw.startsWith('@@')) {
      kind = 'hunk';
      prefix = '';
    } else if (raw.startsWith('+')) {
      kind = 'add';
      prefix = '+';
    } else if (raw.startsWith('-')) {
      kind = 'del';
      prefix = '-';
    } else if (raw.startsWith(' ')) {
      kind = 'ctx';
      prefix = ' ';
    } else {
      kind = 'meta';
      prefix = '';
    }
    const content = prefix ? raw.slice(prefix.length) : raw;
    current.lines.push({ kind, content, prefix });
  }

  // Drop trailing empty block (can happen if diff ends with a lone "diff --git").
  return blocks.filter((b) => b.lines.length > 0);
}

/** Count add/del lines for the stats display. */
function countStats(lines: DiffLine[]): { adds: number; dels: number } {
  let adds = 0;
  let dels = 0;
  for (const l of lines) {
    if (l.kind === 'add') adds++;
    else if (l.kind === 'del') dels++;
  }
  return { adds, dels };
}

interface Props {
  diff: string;
  className?: string;
}

/** Render a unified diff with prism syntax highlighting per file block.
 *
 *  Parsing + classification is pure (parseDiffBlocks); rendering uses prism
 *  to tokenize contiguous code segments. Diff-line backgrounds overlay prism's
 *  token colors via CSS — see .diff-line-* classes in styles.css. */
export function DiffText({ diff, className }: Props) {
  if (!diff.trim()) {
    return <div className={`diff-text-empty ${className ?? ''}`}>(empty diff)</div>;
  }
  const blocks = parseDiffBlocks(diff);
  return (
    <div className={`diff-text ${className ?? ''}`} data-testid="diff-text">
      {blocks.map((block, i) => {
        const { adds, dels } = countStats(block.lines);
        return (
          <div key={i} className="diff-file-block" data-testid="diff-file-block">
            <div className="diff-file-header">
              <span className="diff-file-path">{block.path}</span>
              <span className="diff-file-stats">
                <span className="diff-file-stats-add">+{adds}</span>{' '}
                <span className="diff-file-stats-del">-{dels}</span>
              </span>
            </div>
            <DiffBlockBody block={block} />
          </div>
        );
      })}
    </div>
  );
}

/** Render the lines of one file block. prism tokenizes the whole block body
 *  (concatenated content) as a single language; we map tokens back to lines
 *  for per-line background coloring. */
function DiffBlockBody({ block }: { block: DiffBlock }) {
  // Concatenate content lines (with newlines) for prism. We exclude hunk/meta
  // lines from prism tokenization (they're not code) and render them plain.
  // For simplicity and to preserve multi-line constructs, we feed prism the
  // full sequence of content lines (add + del + ctx interleaved) joined with
  // newlines. Prism doesn't care about the diff prefix.
  const codeLines = block.lines.map((l) => l.content);
  const code = codeLines.join('\n');
  return (
    <Highlight code={code} language={block.lang} theme={themes.nightOwl}>
      {({ tokens, getLineProps, getTokenProps }) => (
        <div className="diff-block-lines">
          {block.lines.map((line, i) => {
            // prism's tokens array aligns 1:1 with our codeLines (we fed it
            // the same line set).
            const prismLine = tokens[i] ?? [];
            const lineProps = getLineProps({ line: prismLine });
            return (
              <div
                key={i}
                className={`diff-line diff-line-${line.kind}`}
                data-testid={`diff-line-${line.kind}`}
              >
                <span className="diff-line-gutter" />
                <span {...lineProps} className="diff-line-content">
                  {prismLine.map((token, key) => (
                    <span key={key} {...getTokenProps({ token })} />
                  ))}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </Highlight>
  );
}
