import { describe, expect, it } from 'vitest';
import {
    buildErrorDialogOptions,
    extractErrorMessage,
    getErrorDetails,
    normalizeError,
} from '../errorUtils';

describe('errorUtils', () => {
    const t = (key: string, options?: Record<string, unknown>) => {
        if (key === 'errors.translation.failed') return 'Translation failed.';
        if (key === 'errors.common.operation_failed') return 'Operation failed.';
        if (key === 'common.error') return 'Error';
        return String(options?.defaultValue ?? key);
    };

    it('normalizes tauri invoke errors to the root cause', () => {
        const error = new Error('error invoking command: something bad happened\nCaused by: network timeout');

        expect(normalizeError(error)).toEqual({
            code: undefined,
            message: 'network timeout',
        });
    });

    it('extracts nested object messages and codes', () => {
        const error = {
            code: 'E_CONN',
            error: {
                message: 'connection reset',
            },
        };

        expect(normalizeError(error)).toEqual({
            code: 'E_CONN',
            message: 'connection reset',
        });
        expect(getErrorDetails(error)).toBe('connection reset [E_CONN]');
    });

    it('falls back to unknown error when no message exists', () => {
        expect(extractErrorMessage({})).toBe('Unknown error');
    });

    it('builds dialog options with optional details', () => {
        expect(buildErrorDialogOptions(t, {
            code: 'translation.failed',
            messageKey: 'errors.translation.failed',
            cause: new Error('timeout'),
        })).toEqual({
            title: 'Error',
            message: 'Translation failed.',
            details: 'timeout',
        });
    });

    it('suppresses details when showCause is disabled', () => {
        expect(buildErrorDialogOptions(t, {
            code: 'translation.failed',
            messageKey: 'errors.translation.failed',
            cause: new Error('timeout'),
            showCause: false,
        })).toEqual({
            title: 'Error',
            message: 'Translation failed.',
            details: undefined,
        });
    });
});
