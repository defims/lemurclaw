import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
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

  it('shows enable/disable action button matching current state', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        {
          cwd: '/repo',
          skills: [
            { name: 'pdf', description: 'PDF', path: { path: '/p' }, scope: 'project', enabled: true } as never,
            { name: 'docx', description: 'DOCX', path: { path: '/d' }, scope: 'project', enabled: false } as never,
          ],
          errors: [],
        },
      ],
    });
    render(<SkillsPanel />);
    await waitFor(() => expect(screen.getByText('pdf')).toBeInTheDocument());
    // enabled skill offers "disable"; disabled skill offers "enable".
    expect(screen.getByRole('button', { name: /disable/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /enable/i })).toBeInTheDocument();
  });

  it('toggling fires skills/config/write with name + new enabled and refetches', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({
        data: [
          {
            cwd: '/repo',
            skills: [
              { name: 'pdf', description: 'PDF', path: { path: '/p' }, scope: 'project', enabled: false } as never,
            ],
            errors: [],
          },
        ],
      })
      .mockResolvedValueOnce({ effectiveEnabled: true }) // skills/config/write ack
      .mockResolvedValueOnce({ // refetch -> now enabled
        data: [
          {
            cwd: '/repo',
            skills: [
              { name: 'pdf', description: 'PDF', path: { path: '/p' }, scope: 'project', enabled: true } as never,
            ],
            errors: [],
          },
        ],
      });
    render(<SkillsPanel />);
    await waitFor(() => expect(screen.getByText('pdf')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /enable/i }));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('skills/config/write', { name: 'pdf', enabled: true });
    });
    // After refetch the button label flips to "disable".
    await waitFor(() => expect(screen.getByRole('button', { name: /disable/i })).toBeInTheDocument());
  });
});
