import { render, screen, waitFor } from '@testing-library/react';
import { LiveCaptionWindow } from '../LiveCaptionWindow';
import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/window', () => ({
    getCurrentWindow: () => ({
        listen: vi.fn().mockImplementation((_event, _cb) => {
            return Promise.resolve(() => {});
        }),
        setIgnoreCursorEvents: vi.fn(),
        emit: vi.fn(),
    })
}));

describe('LiveCaptionWindow', () => {
    it('renders initial state', async () => {
        render(<LiveCaptionWindow />);
        // Wait for async useEffect
        await waitFor(() => {
            expect(screen.getByText('Ready for captioning...')).toBeTruthy();
        });
    });
});
