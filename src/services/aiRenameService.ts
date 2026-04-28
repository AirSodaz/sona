import { invoke } from '@tauri-apps/api/core';
import { historyService } from './historyService';
import { useConfigStore } from '../stores/configStore';
import { getFeatureLlmConfig, isSummaryLlmConfigComplete } from './llm/runtime';
import { normalizeError } from '../utils/errorUtils';
import i18n from '../i18n';
import { TranscriptSegment } from '../types/transcript';
import type { LlmGenerateCommandRequest } from '../types/dashboard';

/**
 * Constructs a prompt for the AI to generate a title based on a transcript snippet.
 */
function buildPrompt(textSnippet: string): string {
    const language = i18n.language.startsWith('zh') ? 'Chinese' : 'English';
    return `You are a helpful assistant that generates short, descriptive, and concise titles for transcriptions. 
Based on the following transcript excerpt, generate a title (max 5 words). 
The title should be in ${language}.
Do not include quotes, prefixes like "Title:", or any other extra text. Just return the title itself.

Transcript:
${textSnippet}`;
}

/**
 * Generates an AI title based on transcript segments.
 *
 * @param segments The transcript segments to analyze.
 * @returns A promise that resolves to the generated title.
 */
export async function generateAiTitle(segments: TranscriptSegment[]): Promise<string> {
    const config = useConfigStore.getState().config;
    const summaryEnabled = config.summaryEnabled ?? true;

    if (!summaryEnabled || !isSummaryLlmConfigComplete(config)) {
        throw new Error(i18n.t('summary.config_error', { defaultValue: 'LLM is not configured or disabled.' }));
    }

    const llmConfig = getFeatureLlmConfig(config, 'summary');
    if (!llmConfig) {
        throw new Error(i18n.t('summary.config_error', { defaultValue: 'LLM is not configured.' }));
    }

    if (!segments || segments.length === 0) {
        throw new Error(i18n.t('history.no_transcript', { defaultValue: 'No transcript available.' }));
    }

    // Extract first 1500 characters from segments
    const textSnippet = segments.slice(0, 50).map(s => s.text).join(' ').slice(0, 1500);
    const prompt = buildPrompt(textSnippet);

    try {
        const title = await invoke<string>('generate_llm_text', {
            request: {
                config: llmConfig,
                input: prompt,
                source: 'title_generation',
            } satisfies LlmGenerateCommandRequest,
        });
        // Basic cleanup: remove surrounding quotes and extra whitespace
        return title.trim().replace(/^["']|["']$/g, '');
    } catch (error) {
        throw new Error(normalizeError(error).message);
    }
}

/**
 * Generates an AI title for a history item by loading its transcript file first.
 *
 * @param transcriptPath The path to the transcript JSON file.
 * @returns A promise that resolves to the generated title.
 */
export async function generateAiTitleForHistoryItem(transcriptPath: string): Promise<string> {
    try {
        const segments = await historyService.loadTranscript(transcriptPath);
        if (!segments) {
            throw new Error(i18n.t('history.no_transcript', { defaultValue: 'No transcript available.' }));
        }
        return await generateAiTitle(segments);
    } catch (error) {
        throw new Error(normalizeError(error).message);
    }
}
