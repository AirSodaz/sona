import type React from 'react';
import { ModelManagerContext, useModelManager } from '../../hooks/useModelManager';
import { type ModelScenario, SettingsModelsTab } from './SettingsModelsTab';

export type { ModelScenario };

interface SettingsModelsPaneProps {
  isOpen: boolean;
  isActive?: boolean;
  initialScenario?: ModelScenario;
  subTabRequestId?: number;
}

export function SettingsModelsPane({
  isOpen,
  isActive = isOpen,
  initialScenario,
  subTabRequestId,
}: SettingsModelsPaneProps): React.JSX.Element {
  const shouldRunActiveEffects = isOpen && isActive;
  const modelManager = useModelManager(shouldRunActiveEffects);

  return (
    <ModelManagerContext.Provider value={modelManager}>
      <SettingsModelsTab
        isActive={shouldRunActiveEffects}
        initialScenario={initialScenario}
        subTabRequestId={subTabRequestId}
      />
    </ModelManagerContext.Provider>
  );
}

export default SettingsModelsPane;
