import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CommandExecCell } from '../CommandExecCell';

describe('CommandExecCell', () => {
  it('shows command + status, hides output until expanded', () => {
    render(<CommandExecCell model={{ kind: 'commandExecution', itemId: 'e1', command: 'cargo build', cwd: '/proj', status: 'completed', source: 'agent', aggregatedOutput: 'Compiling...', exitCode: 0, durationMs: 1234 }} />);
    expect(screen.getByText(/cargo build/)).toBeInTheDocument();
    expect(screen.getByText('✓ exit 0')).toBeInTheDocument();
    expect(screen.queryByText('Compiling...')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: /show output/ }));
    expect(screen.getByTestId('exec-output')).toHaveTextContent('Compiling...');
  });

  it('marks failed status with non-zero exit', () => {
    render(<CommandExecCell model={{ kind: 'commandExecution', itemId: 'e2', command: 'false', cwd: '/x', status: 'failed', source: 'agent', aggregatedOutput: '', exitCode: 1, durationMs: 10 }} />);
    expect(screen.getByText('✗ exit 1')).toBeInTheDocument();
  });
});
