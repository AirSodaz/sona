import React from 'react';
import { ModelManagerContext, useModelManager } from '../../hooks/useModelManager';
import { SettingsModelsTab } from './SettingsModelsTab';

interface SettingsModelsPaneProps {
    isOpen: boolean;
}

export function SettingsModelsPane({ isOpen }: SettingsModelsPaneProps): React.JSX.Element {
    const modelManager = useModelManager(isOpen);

    return (
        <ModelManagerContext.Provider value={modelManager}>
            <SettingsModelsTab />
        </ModelManagerContext.Provider>
    );
}
