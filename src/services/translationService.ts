import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import { historyService } from './historyService';
import { logger } from '../utils/logger';

class TranslationService {
    /**
     * Translates the current segments in the store using the configured LLM service.
     * Updates the store's segments progressively.
     */
    async translateCurrentTranscript() {
        const store = useTranscriptStore.getState();
        const config = store.config;

        if (!config.llmApiKey || !config.llmBaseUrl || !config.llmModel || !config.llmServiceType) {
            throw new Error('LLM Service not fully configured.');
        }

        const segments = store.segments;
        if (!segments || segments.length === 0) {
            return;
        }

        const jobHistoryId = store.sourceHistoryId || 'current';

        store.updateLlmState({ isTranslating: true, translationProgress: 0 }, jobHistoryId);

        const CHUNK_SIZE = 30; // Number of segments to translate per API request
        const totalChunks = Math.ceil(segments.length / CHUNK_SIZE);

        try {
            for (let i = 0; i < totalChunks; i++) {
                const chunk = segments.slice(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);

                // Prepare prompt
                const prompt = this.buildPrompt(chunk, config.translationLanguage || 'zh');

                // Call LLM
                const responseText = await invoke<string>('call_llm_model', {
                    apiKey: config.llmApiKey,
                    baseUrl: config.llmBaseUrl,
                    modelName: config.llmModel,
                    input: prompt,
                    apiFormat: config.llmServiceType,
                    temperature: config.llmTemperature ?? 0.7,
                });

                // Parse JSON output
                const translations = this.parseLlmResponse(responseText);

                const currentStore = useTranscriptStore.getState();
                const currentHistoryId = currentStore.sourceHistoryId || 'current';

                if (currentHistoryId === jobHistoryId) {
                    // Still on the same record, update store directly
                    translations.forEach(({ id, translation }) => {
                        currentStore.updateSegment(id, { translation });
                    });
                } else if (jobHistoryId !== 'current') {
                    // User switched to another record. We must update the background record's file directly.
                    try {
                        // Load the background record's segments from file
                        const bgSegments = await historyService.loadTranscript(`${jobHistoryId}.json`);
                        if (bgSegments) {
                            // Update the segments with translations
                            translations.forEach(({ id, translation }) => {
                                const seg = bgSegments.find(s => s.id === id);
                                if (seg) seg.translation = translation;
                            });
                            // Save back to file
                            await historyService.updateTranscript(jobHistoryId, bgSegments);
                        }
                    } catch (e) {
                        logger.error('[TranslationService] Failed to update background record segments:', e);
                    }
                }

                // Update progress for the specific record
                const progress = Math.round(((i + 1) / totalChunks) * 100);
                useTranscriptStore.getState().updateLlmState({ translationProgress: progress }, jobHistoryId);
            }
        } finally {
            const currentStore = useTranscriptStore.getState();
            currentStore.updateLlmState({ isTranslating: false, translationProgress: 100 }, jobHistoryId);

            // Auto-show translations when done if not visible
            if (!store.getLlmState(jobHistoryId).isTranslationVisible) {
                store.updateLlmState({ isTranslationVisible: true }, jobHistoryId);
            }
        }
    }

    private buildPrompt(segments: { id: string; text: string }[], targetLanguage: string): string {
        const jsonStr = JSON.stringify(segments.map(s => ({ id: s.id, text: s.text })));
        return `You are a professional translator. Translate the following array of text segments into ${this.getLanguageName(targetLanguage)}.
CRITICAL INSTRUCTIONS:
1. You MUST maintain the EXACT JSON array structure.
2. The output MUST be valid JSON and ONLY valid JSON. Do not include markdown formatting like \`\`\`json.
3. Return an array of objects with the EXACT SAME 'id' field, but replace 'text' with 'translation'.
4. Do not combine or split segments. There must be exactly ${segments.length} objects in the output.

Input:
${jsonStr}`;
    }

    private parseLlmResponse(responseText: string): { id: string; translation: string }[] {
        try {
            // Sometimes the LLM includes markdown wrapping even when told not to.
            // Strip ```json and ```
            let cleaned = responseText.trim();
            if (cleaned.startsWith('```json')) cleaned = cleaned.substring(7);
            if (cleaned.startsWith('```')) cleaned = cleaned.substring(3);
            if (cleaned.endsWith('```')) cleaned = cleaned.substring(0, cleaned.length - 3);
            cleaned = cleaned.trim();

            const parsed = JSON.parse(cleaned);
            if (!Array.isArray(parsed)) {
                throw new Error("LLM response is not a valid JSON array");
            }
            return parsed;
        } catch (e) {
            console.error("Failed to parse LLM translation response:", e, "\nRaw Response:", responseText);
            throw new Error(`Failed to parse LLM response: ${e instanceof Error ? e.message : 'Unknown'}`);
        }
    }

    private getLanguageName(code: string): string {
        const map: Record<string, string> = {
            'zh': 'Chinese (Simplified)',
            'en': 'English',
            'ja': 'Japanese',
            'ko': 'Korean',
            'fr': 'French',
            'de': 'German',
            'es': 'Spanish',
        };
        return map[code] || code;
    }
}

export const translationService = new TranslationService();
