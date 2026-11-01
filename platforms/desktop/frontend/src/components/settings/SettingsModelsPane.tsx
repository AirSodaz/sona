import type React from 'react';
import { ModelManagerContext, useModelManager } from '../../hooks/useModelManager';
import type { ModelScenario } from './SettingsModelsTab';
import { SettingsSpeechAiTab } from './speechAi/SettingsSpeechAiTab';

export type { ModelScenario };

interface SettingsModelsPaneProps {
  isOpen: boolean;
  isActive?: boolean;
  initialScenario?: ModelScenario;
  subTabRequestId?: number;
  speechEntry?: { view: string; requestId: number } | null;
}

export function SettingsModelsPane({
  isOpen,
  isActive = isOpen,
  initialScenario,
  subTabRequestId,
  speechEntry,
}: SettingsModelsPaneProps): React.JSX.Element {
  const shouldRunActiveEffects = isOpen && isActive;
  const modelManager = useModelManager(shouldRunActiveEffects);
  const entry =
    speechEntry ??
    (initialScenario ? { view: 'recognition', requestId: subTabRequestId ?? 1 } : null);

  return (
    <ModelManagerContext.Provider value={modelManager}>
      <SettingsSpeechAiTab speechEntry={entry} />
    </ModelManagerContext.Provider>
  );
}

export default SettingsModelsPane;
