import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import { TranscriptSegment } from '../types/transcript';
import { POLISH_SCENARIO_PROMPTS } from '../utils/polishPrompts';
import { historyService } from './historyService';
import { logger } from '../utils/logger';

class PolishService {
    /**
     * Polishes the provided segments using the configured AI service.
     *
     * @param segments The list of segments to polish.
     * @param onChunkPolished Optional callback when a chunk of segments is polished.
     *                        If not provided, no side effects occur (store is not updated).
     * @returns A promise that resolves when all segments are polished.
     */
    async polishSegments(
        segments: TranscriptSegment[],
        onChunkPolished?: (polishedChunk: { id: string; text: string }[]) => void
    ): Promise<void> {
        const store = useTranscriptStore.getState();
        const config = store.config;

        if (!config.aiApiKey || !config.aiBaseUrl || !config.aiModel || !config.aiServiceType) {
            // If AI is not configured, we might want to skip polishing silently or throw error.
            // For auto-polish, skipping silently or logging warning is better than crashing.
            // However, manual polish expects an error.
            // Let's throw, and let the caller handle it.
            throw new Error('AI Service not fully configured.');
        }

        if (!segments || segments.length === 0) {
            return;
        }

        const CHUNK_SIZE = 30; // Number of segments to polish per API request
        const totalChunks = Math.ceil(segments.length / CHUNK_SIZE);

        for (let i = 0; i < totalChunks; i++) {
            const chunk = segments.slice(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);

            // Prepare prompt
            const prompt = this.buildPrompt(chunk);

            try {
                // Call AI
                const responseText = await invoke<string>('call_ai_model', {
                    apiKey: config.aiApiKey,
                    baseUrl: config.aiBaseUrl,
                    modelName: config.aiModel,
                    input: prompt,
                    apiFormat: config.aiServiceType,
                    temperature: config.aiTemperature ?? 0.7,
                });

                // Parse JSON output
                const polishedSegments = this.parseAIResponse(responseText);

                if (onChunkPolished) {
                    onChunkPolished(polishedSegments);
                }
            } catch (error) {
                console.error('Failed to polish chunk:', error);
                throw error; // Re-throw to let caller know
            }
        }
    }

    /**
     * Polishes all segments in the store.
     * Updates the store's segments progressively.
     */
    async polishTranscript() {
        const store = useTranscriptStore.getState();
        const segments = store.segments;

        if (!segments || segments.length === 0) {
            return;
        }

        const jobHistoryId = store.sourceHistoryId || 'current';

        store.updateAiState({ isPolishing: true, polishProgress: 0 }, jobHistoryId);

        const totalChunks = Math.ceil(segments.length / 30);
        let completedChunks = 0;

        try {
            await this.polishSegments(segments, async (polishedChunk) => {
                const currentStore = useTranscriptStore.getState();
                const currentHistoryId = currentStore.sourceHistoryId || 'current';

                if (currentHistoryId === jobHistoryId) {
                    polishedChunk.forEach(({ id, text }) => {
                        currentStore.updateSegment(id, { text });
                    });
                } else if (jobHistoryId !== 'current') {
                    // Update background record's file directly
                    try {
                        const bgSegments = await historyService.loadTranscript(`${jobHistoryId}.json`);
                        if (bgSegments) {
                            const segMap = new Map<string, TranscriptSegment>();
                            for (let i = 0; i < bgSegments.length; i++) {
                                const seg = bgSegments[i];
                                segMap.set(seg.id, seg);
                            }

                            polishedChunk.forEach(({ id, text }) => {
                                const seg = segMap.get(id);
                                if (seg) seg.text = text;
                            });
                            await historyService.updateTranscript(jobHistoryId, bgSegments);
                        }
                    } catch (e) {
                        logger.error('[PolishService] Failed to update background record segments:', e);
                    }
                }

                completedChunks++;
                useTranscriptStore.getState().updateAiState({
                    polishProgress: Math.round((completedChunks / totalChunks) * 100)
                }, jobHistoryId);
            });
        } finally {
            useTranscriptStore.getState().updateAiState({
                isPolishing: false,
                polishProgress: 0
            }, jobHistoryId);
        }
    }

    private buildPrompt(segments: { id: string; text: string }[]): string {
        const config = useTranscriptStore.getState().config;
        const jsonStr = JSON.stringify(segments.map(s => ({ id: s.id, text: s.text })));

        let prompt = "";

        let contextText = "";
        const scenario = config.polishScenario || 'custom';

        if (scenario === 'custom') {
            contextText = config.polishContext || "";
        } else {
            contextText = POLISH_SCENARIO_PROMPTS[scenario] || "";
        }

        if (contextText.trim()) {
            prompt += `[User Context]\n${contextText.trim()}\n\n`;
        }

        if (config.polishKeywords && config.polishKeywords.trim()) {
            prompt += `[User Keywords]\n${config.polishKeywords.trim()}\n\n`;
        }

        prompt += `You are a professional editor. The following text segments are from a speech-to-text transcription and may contain errors.
Your task is to:
1. Fix any speech recognition errors.
2. Improve grammar and clarity.
3. Keep the meaning unchanged.
4. Do NOT translate. Keep the original language.

CRITICAL INSTRUCTIONS:
1. You MUST maintain the EXACT JSON array structure.
2. The output MUST be valid JSON and ONLY valid JSON. Do not include markdown formatting like \`\`\`json.
3. Return an array of objects with the EXACT SAME 'id' field, and the polished text in the 'text' field.
4. Do not combine or split segments. There must be exactly ${segments.length} objects in the output.

Input:
${jsonStr}`;

        return prompt;
    }

    private parseAIResponse(responseText: string): { id: string; text: string }[] {
        try {
            let cleaned = responseText.trim();
            if (cleaned.startsWith('```json')) cleaned = cleaned.substring(7);
            else if (cleaned.startsWith('```')) cleaned = cleaned.substring(3);

            if (cleaned.endsWith('```')) cleaned = cleaned.substring(0, cleaned.length - 3);

            cleaned = cleaned.trim();

            const parsed = JSON.parse(cleaned);
            if (!Array.isArray(parsed)) {
                throw new Error("AI response is not a valid JSON array");
            }
            return parsed;
        } catch (e) {
            console.error("Failed to parse AI polish response:", e, "\nRaw Response:", responseText);
            throw new Error(`Failed to parse AI response: ${e instanceof Error ? e.message : 'Unknown'}`);
        }
    }
}

export const polishService = new PolishService();
