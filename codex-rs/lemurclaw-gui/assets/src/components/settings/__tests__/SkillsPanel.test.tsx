import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { SkillsPanel } from '../SkillsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('SkillsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists skills from skills/list flattened across cwds', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        {
          cwd: '/repo',
          skills: [
            { name: 'pdf', description: 'PDF skill', path: { path: '/repo/skills/pdf' }, scope: 'project', enabled: true } as never,
            { name: 'docx', description: 'DOCX skill', path: { path: '/repo/skills/docx' }, scope: 'project', enabled: false } as never,
          ],
          errors: [],
        },
      ],
    });
    render(<SkillsPanel />);
    await waitFor(() => expect(screen.getByText('pdf')).toBeInTheDocument());
    expect(screen.getByText('docx')).toBeInTheDocument();
  });

  it('shows empty state when no skills discovered', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [] });
    render(<SkillsPanel />);
    await waitFor(() => expect(screen.getByText(/no skills discovered/i)).toBeInTheDocument());
  });
});
