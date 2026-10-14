import { logger } from '../utils/logger';

class EffectiveConfigRuntime {
  private started = false;

  init() {
    if (this.started) {
      return;
    }
    this.started = true;
    logger.debug?.('[EffectiveConfig] Runtime initialized');
  }

  stop() {
    this.started = false;
    logger.debug?.('[EffectiveConfig] Runtime stopped');
  }
}

export const effectiveConfigRuntime = new EffectiveConfigRuntime();
