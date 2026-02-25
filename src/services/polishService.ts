import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';

class PolishService {
    /**
     * Polishes the current segments in the store using the configured AI service.
     * Updates the store's segments progressively.
     */
    async polishTranscript() {
        const store = useTranscriptStore.getState();
        const config = store.config;

        if (!config.aiApiKey || !config.aiBaseUrl || !config.aiModel || !config.aiServiceType) {
            throw new Error('AI Service not fully configured.');
        }

        const segments = store.segments;
        if (!segments || segments.length === 0) {
            return;
        }

        store.setIsPolishing(true);
        store.setPolishProgress(0);

        const CHUNK_SIZE = 30; // Number of segments to polish per API request
        const totalChunks = Math.ceil(segments.length / CHUNK_SIZE);

        try {
            for (let i = 0; i < totalChunks; i++) {
                const chunk = segments.slice(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);

                // Prepare prompt
                const prompt = this.buildPrompt(chunk);

                // Call AI
                const responseText = await invoke<string>('call_ai_model', {
                    apiKey: config.aiApiKey,
                    baseUrl: config.aiBaseUrl,
                    modelName: config.aiModel,
                    input: prompt,
                    apiFormat: config.aiServiceType,
                });

                // Parse JSON output
                const polishedSegments = this.parseAIResponse(responseText);

                // Update the store
                const currentStore = useTranscriptStore.getState();
                polishedSegments.forEach(({ id, text }) => {
                    currentStore.updateSegment(id, { text });
                });

                // Update progress
                store.setPolishProgress(Math.round(((i + 1) / totalChunks) * 100));
            }
        } finally {
            store.setIsPolishing(false);
            store.setPolishProgress(0);
        }
    }

    private buildPrompt(segments: { id: string; text: string }[]): string {
        const config = useTranscriptStore.getState().config;
        const jsonStr = JSON.stringify(segments.map(s => ({ id: s.id, text: s.text })));

        let prompt = "";

        if (config.polishContext && config.polishContext.trim()) {
            prompt += `[User Context]\n${config.polishContext.trim()}\n\n`;
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
