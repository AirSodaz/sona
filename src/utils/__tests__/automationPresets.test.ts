import { describe, expect, it } from 'vitest';
import {
    applyAutomationPreset,
    findMatchingAutomationPreset,
    isBuiltInAutomationPresetId,
    matchesAutomationPreset,
} from '../automationPresets';

describe('automationPresets', () => {
    it('treats custom as a non-built-in preset id', () => {
        expect(isBuiltInAutomationPresetId('meeting_notes')).toBe(true);
        expect(isBuiltInAutomationPresetId('bilingual_subtitles')).toBe(true);
        expect(isBuiltInAutomationPresetId('custom')).toBe(false);
        expect(isBuiltInAutomationPresetId('unknown')).toBe(false);
    });

    it('matches and finds built-in templates from the five template-controlled fields', () => {
        const bilingualConfig = applyAutomationPreset('bilingual_subtitles', {
            stageConfig: {
                autoPolish: true,
                autoTranslate: false,
                exportEnabled: true,
            },
            exportConfig: {
                directory: 'C:\\exports',
                format: 'txt',
                mode: 'original',
            },
        });

        expect(matchesAutomationPreset('bilingual_subtitles', bilingualConfig)).toBe(true);
        expect(matchesAutomationPreset('meeting_notes', bilingualConfig)).toBe(false);
        expect(findMatchingAutomationPreset(bilingualConfig)).toBe('bilingual_subtitles');
    });

    it('returns null when the template-controlled fields no longer match a built-in preset exactly', () => {
        expect(findMatchingAutomationPreset({
            stageConfig: {
                autoPolish: true,
                autoTranslate: true,
                exportEnabled: true,
            },
            exportConfig: {
                directory: 'C:\\exports',
                format: 'txt',
                mode: 'translation',
            },
        })).toBeNull();
    });
});
