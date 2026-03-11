import { render, screen, fireEvent } from '@testing-library/react';
import { Checkbox } from '../Checkbox';
import React from 'react';
import { test, expect } from 'vitest';

test('renders checkbox with correct aria attributes', () => {
    const TestComponent = () => {
        const [checked, setChecked] = React.useState(false);
        return <Checkbox checked={checked} onChange={setChecked} label="Test Label" aria-label="Custom ARIA" />;
    };

    const result = render(<TestComponent />);

    const div = result.container.querySelector('div') as HTMLElement;

    expect(div.getAttribute('role')).toBe('checkbox');
    expect(div.getAttribute('aria-checked')).toBe('false');
    expect(div.getAttribute('aria-label')).toBe('Custom ARIA');
    expect(div.getAttribute('tabindex')).toBe('0');
});
