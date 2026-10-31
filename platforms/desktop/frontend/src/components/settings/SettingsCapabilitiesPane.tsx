import type React from 'react';
import { ModelManagerContext, useModelManager } from '../../hooks/useModelManager';
import type { LlmProvider } from '../../types/transcript';
import {
  type CapabilitiesFocus,
  SettingsCapabilitiesTab,
} from './SettingsCapabilitiesTab';

interface SettingsCapabilitiesPaneProps {
  isOpen: boolean;
  isActive?: boolean;
  initialFocus?: CapabilitiesFocus;
  focusRequestId?: number;
  onOpenLlmProviderDetails?: (provider: LlmProvider) => void;
}

export function SettingsCapabilitiesPane({
  isOpen,
  isActive = isOpen,
  initialFocus,
  focusRequestId,
  onOpenLlmProviderDetails,
}: SettingsCapabilitiesPaneProps): React.JSX.Element {
  const shouldRunActiveEffects = isOpen && isActive;
  const modelManager = useModelManager(shouldRunActiveEffects);

  return (
    <ModelManagerContext.Provider value={modelManager}>
      <SettingsCapabilitiesTab
        isActive={shouldRunActiveEffects}
        initialFocus={initialFocus}
        focusRequestId={focusRequestId}
        onOpenLlmProviderDetails={onOpenLlmProviderDetails}
      />
    </ModelManagerContext.Provider>
  );
}

export default SettingsCapabilitiesPane;
