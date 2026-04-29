import { resolveEffectiveConfig } from './effectiveConfigService';
import { useConfigStore } from '../stores/configStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptStore } from '../stores/transcriptStore';

function syncTranscriptConfig() {
  const projectState = useProjectStore.getState();
  const activeProject = typeof projectState.getActiveProject === 'function'
    ? projectState.getActiveProject()
    : null;

  useTranscriptStore.setState({
    config: resolveEffectiveConfig(useConfigStore.getState().config, activeProject),
  });
}

class TranscriptConfigRuntime {
  private started = false;

  private unsubscribeConfig: (() => void) | null = null;

  private unsubscribeProject: (() => void) | null = null;

  init() {
    if (this.started) {
      syncTranscriptConfig();
      return;
    }

    this.started = true;
    syncTranscriptConfig();

    this.unsubscribeConfig = useConfigStore.subscribe(() => {
      syncTranscriptConfig();
    });

    if (typeof useProjectStore.subscribe === 'function') {
      this.unsubscribeProject = useProjectStore.subscribe(() => {
        syncTranscriptConfig();
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

export const transcriptConfigRuntime = new TranscriptConfigRuntime();
