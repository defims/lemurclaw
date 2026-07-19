import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { PlanCell } from '../PlanCell';

describe('PlanCell', () => {
  it('renders plan text', () => {
    render(<PlanCell model={{ kind: 'plan', itemId: 'pl1', text: '1. do thing\n2. done' }} />);
    expect(screen.getByTestId('plan')).toHaveTextContent('1. do thing');
  });
});
