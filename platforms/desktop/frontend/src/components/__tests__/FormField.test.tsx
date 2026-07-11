import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { FormField } from '../FormField';
import { Switch } from '../Switch';

describe('FormField', () => {
  it('uses its label as the accessible name for a custom switch control', () => {
    render(
      <FormField id="timeline-mode" label="Timeline mode">
        <Switch id="timeline-mode" checked={false} onChange={vi.fn()} />
      </FormField>,
    );

    expect(screen.getByRole('switch', { name: 'Timeline mode' })).toBeDefined();
  });
});
