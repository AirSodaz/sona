import type { AppConfig } from '../../types/config';
import type { TranslatedSegment } from '../../types/llmTask';
import type { TranscriptSegment } from '../../types/transcript';
import {
  translationService as defaultTranslationService,
  type TranslationService,
} from '../translationService';

export interface StreamingTranslationSchedulerOptions {
  config: AppConfig;
  targetLanguage: string;
  batchSize?: number;
  maxWaitMs?: number;
  maxConcurrency?: number;
  onBatchTranslated: (translated: TranslatedSegment[]) => Promise<void> | void;
  onError?: (error: unknown) => void;
  isCancelRequested: () => boolean;
  translationService?: Pick<TranslationService, 'translateSegmentsWithConfig'>;
}

export class StreamingTranslationScheduler {
  private queue: TranscriptSegment[] = [];
  private pushedIds: Set<string> = new Set();
  private translationsById: Map<string, string> = new Map();
  private activeConcurrency = 0;
  private consecutiveErrors = 0;
  private circuitBroken = false;
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  private inFlightPromises: Set<Promise<void>> = new Set();
  private cancelled = false;

  private readonly batchSize: number;
  private readonly maxWaitMs: number;
  private readonly maxConcurrency: number;
  private readonly translationService: Pick<TranslationService, 'translateSegmentsWithConfig'>;

  constructor(private readonly options: StreamingTranslationSchedulerOptions) {
    this.batchSize = options.batchSize ?? 6;
    this.maxWaitMs = options.maxWaitMs ?? 2500;
    this.maxConcurrency = options.maxConcurrency ?? 2;
    this.translationService = options.translationService ?? defaultTranslationService;
  }

  get isCircuitBroken(): boolean {
    return this.circuitBroken;
  }

  get activeConcurrencyCount(): number {
    return this.activeConcurrency;
  }

  get queueLength(): number {
    return this.queue.length;
  }

  push(segment: TranscriptSegment): void {
    if (this.cancelled || this.options.isCancelRequested()) {
      return;
    }
    if (!segment.id || this.pushedIds.has(segment.id)) {
      return;
    }

    if (segment.translation?.trim()) {
      this.pushedIds.add(segment.id);
      this.translationsById.set(segment.id, segment.translation);
      return;
    }

    if (!segment.text?.trim()) {
      this.pushedIds.add(segment.id);
      return;
    }

    this.pushedIds.add(segment.id);
    this.queue.push(segment);

    if (this.circuitBroken) {
      return;
    }

    if (this.queue.length >= this.batchSize) {
      void this.flush();
    } else if (!this.flushTimer) {
      this.flushTimer = setTimeout(() => {
        this.flushTimer = null;
        void this.flush(true);
      }, this.maxWaitMs);
    }
  }

  async flush(forceTrailing = false): Promise<void> {
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }

    if (this.circuitBroken || this.cancelled || this.options.isCancelRequested()) {
      return;
    }

    while (
      this.queue.length > 0 &&
      this.activeConcurrency < this.maxConcurrency &&
      (this.queue.length >= this.batchSize || forceTrailing) &&
      !this.circuitBroken &&
      !this.cancelled &&
      !this.options.isCancelRequested()
    ) {
      const count = Math.min(this.queue.length, this.batchSize);
      const batch = this.queue.splice(0, count);
      this.activeConcurrency += 1;

      const { promise: batchPromise, resolve: resolveBatchPromise } = Promise.withResolvers<void>();
      this.inFlightPromises.add(batchPromise);

      void this.executeBatch(batch).finally(() => {
        this.activeConcurrency -= 1;
        this.inFlightPromises.delete(batchPromise);
        resolveBatchPromise();

        if (!this.cancelled && !this.circuitBroken && !this.options.isCancelRequested()) {
          if (this.queue.length >= this.batchSize) {
            void this.flush();
          } else if (forceTrailing && this.queue.length > 0) {
            void this.flush(forceTrailing);
          } else if (this.queue.length > 0 && !this.flushTimer) {
            this.flushTimer = setTimeout(() => {
              this.flushTimer = null;
              void this.flush(true);
            }, this.maxWaitMs);
          }
        }
      });
    }

    if (
      this.queue.length > 0 &&
      !this.flushTimer &&
      !this.circuitBroken &&
      !this.cancelled &&
      !this.options.isCancelRequested()
    ) {
      this.flushTimer = setTimeout(() => {
        this.flushTimer = null;
        void this.flush(true);
      }, this.maxWaitMs);
    }
  }

  private async executeBatch(batch: TranscriptSegment[]): Promise<void> {
    try {
      const translateConfig: Pick<AppConfig, 'llmSettings' | 'translationLanguage'> = {
        ...this.options.config,
        translationLanguage: this.options.targetLanguage,
      };
      const results = await this.translationService.translateSegmentsWithConfig(
        translateConfig,
        batch
      );

      if (this.cancelled || this.options.isCancelRequested()) {
        return;
      }

      this.consecutiveErrors = 0;
      for (const item of results) {
        if (item.translation) {
          this.translationsById.set(item.id, item.translation);
        }
      }
      await this.options.onBatchTranslated(results);
    } catch (error) {
      this.consecutiveErrors += 1;
      try {
        this.options.onError?.(error);
      } catch {
        // Ignore subscriber handler errors to prevent unhandled rejections
      }
      if (this.consecutiveErrors >= 3) {
        this.circuitBroken = true;
      }
    }
  }

  async drain(): Promise<void> {
    try {
      if (this.flushTimer) {
        clearTimeout(this.flushTimer);
        this.flushTimer = null;
      }

      while (
        this.queue.length > 0 &&
        !this.circuitBroken &&
        !this.cancelled &&
        !this.options.isCancelRequested()
      ) {
        if (this.activeConcurrency < this.maxConcurrency) {
          void this.flush(true);
        } else if (this.inFlightPromises.size > 0) {
          await Promise.race(Array.from(this.inFlightPromises));
        } else {
          break;
        }
      }

      if (this.inFlightPromises.size > 0) {
        await Promise.allSettled(Array.from(this.inFlightPromises));
      }
    } finally {
      if (this.flushTimer) {
        clearTimeout(this.flushTimer);
        this.flushTimer = null;
      }
    }
  }

  cancel(): void {
    this.cancelled = true;
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }
    this.queue = [];
  }

  getTranslations(): Map<string, string> {
    return new Map(this.translationsById);
  }
}
