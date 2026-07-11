import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SettingsShortcutsTab } from '../SettingsShortcutsTab';
import { setTestConfig } from '../../../test-utils/configTestUtils';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

describe('SettingsShortcutsTab', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        setTestConfig({
            liveRecordShortcut: 'Ctrl + Space',
        });
    });

    it('keeps only generic shortcut references and removes voice typing controls', () => {
        render(<SettingsShortcutsTab />);

        expect(screen.getByText('shortcuts.record_start_stop')).toBeDefined();
        expect(screen.queryByText('settings.enable_voice_typing')).toBeNull();
        expect(screen.queryByText('settings.voice_typing_shortcut')).toBeNull();
        expect(screen.queryByText('settings.voice_typing_mode')).toBeNull();
    });
});
