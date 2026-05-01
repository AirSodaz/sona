import React from 'react';
import { ModelManagerContext, useModelManager } from '../../hooks/useModelManager';
import { SettingsModelsTab } from './SettingsModelsTab';

interface SettingsModelsPaneProps {
    isOpen: boolean;
    isActive?: boolean;
}

export function SettingsModelsPane({ isOpen, isActive = isOpen }: SettingsModelsPaneProps): React.JSX.Element {
    const shouldRunActiveEffects = isOpen && isActive;
    const modelManager = useModelManager(shouldRunActiveEffects);

    return (
        <ModelManagerContext.Provider value={modelManager}>
            <SettingsModelsTab isActive={shouldRunActiveEffects} />
        </ModelManagerContext.Provider>
    );
}
