import { useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useProjectStore } from '../stores/projectStore';

function syncEffectiveConfig() {
  useEffectiveConfigStore.getState().syncConfig();
}

class EffectiveConfigRuntime {
  private started = false;

  private unsubscribeConfig: (() => void) | null = null;

  private unsubscribeProject: (() => void) | null = null;

  init() {
    if (this.started) {
      syncEffectiveConfig();
      return;
    }

    this.started = true;
    syncEffectiveConfig();

    this.unsubscribeConfig = useConfigStore.subscribe(() => {
      syncEffectiveConfig();
    });

    if (typeof useProjectStore.subscribe === 'function') {
      this.unsubscribeProject = useProjectStore.subscribe(() => {
        syncEffectiveConfig();
      });
    }
  }

  stop() {
    this.unsubscribeConfig?.();
    this.unsubscribeProject?.();
    this.unsubscribeConfig = null;
    this.unsubscribeProject = null;
    this.started = false;
  }
}

export const effectiveConfigRuntime = new EffectiveConfigRuntime();
