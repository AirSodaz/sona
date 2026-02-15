import React, { Dispatch, SetStateAction, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
    DndContext,
    closestCenter,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    DragEndEvent
} from '@dnd-kit/core';
import {
    arrayMove,
    SortableContext,
    sortableKeyboardCoordinates,
    verticalListSortingStrategy,
    useSortable
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { PRESET_MODELS, ModelInfo } from '../../services/modelService';
import { DragHandleIcon, DownloadIcon, XIcon } from '../Icons';

// --- Helper Component: SortableItem ---

interface SortableItemProps {
    id: string;
    children: React.ReactNode;
    style?: React.CSSProperties;
}

function SortableItem({ id, children, style: propStyle }: SortableItemProps): React.JSX.Element {
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
    } = useSortable({ id });

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
        touchAction: 'none' as const, // Prevent scrolling on touch while dragging
        ...propStyle
    };

    return (
        <div ref={setNodeRef} style={style}>
            <div style={{ display: 'flex', alignItems: 'center', width: '100%' }}>
                <div {...attributes} {...listeners} style={{ display: 'flex', alignItems: 'center', paddingRight: 8, cursor: 'grab' }}>
                    <DragHandleIcon />
                </div>
                <div style={{ flex: 1 }}>
                    {children}
                </div>
            </div>
        </div>
    );
}

// --- Main Component: ItnModelList ---

interface ItnModelListProps {
    itnRulesOrder: string[];
    setItnRulesOrder: Dispatch<SetStateAction<string[]>>;
    enabledITNModels: Set<string>;
    setEnabledITNModels: Dispatch<SetStateAction<Set<string>>>;
    installedITNModels: Set<string>;
    // downloadingId: string | null;
    // progress: number;
    downloads: Record<string, { progress: number; status: string }>;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
}

export function ItnModelList({
    itnRulesOrder,
    setItnRulesOrder,
    enabledITNModels,
    setEnabledITNModels,
    installedITNModels,
    downloads,
    onDownload,
    onCancelDownload
}: ItnModelListProps): React.JSX.Element {
    const { t } = useTranslation();

    const sensors = useSensors(
        useSensor(PointerSensor),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates,
        })
    );

    // Compute display order ensuring all ITN models are present
    const displayIds = useMemo(() => {
        const allItnModels = PRESET_MODELS.filter(m => m.type === 'itn');
        const allItnIds = new Set(allItnModels.map(m => m.id));

        // Filter out obsolete IDs from order
        const validOrder = itnRulesOrder.filter(id => allItnIds.has(id));

        // Add missing IDs
        const missingIds = allItnModels.filter(m => !validOrder.includes(m.id)).map(m => m.id);

        return [...validOrder, ...missingIds];
    }, [itnRulesOrder]);


    function handleDragEnd(event: DragEndEvent) {
        const { active, over } = event;

        if (over && active.id !== over.id) {
            const oldIndex = displayIds.indexOf(String(active.id));
            const newIndex = displayIds.indexOf(String(over.id));
            const newOrder = arrayMove(displayIds, oldIndex, newIndex);
            setItnRulesOrder(newOrder);
        }
    }

    function toggleModel(modelId: string) {
        const next = new Set(enabledITNModels);
        if (next.has(modelId)) next.delete(modelId);
        else next.add(modelId);
        setEnabledITNModels(next);
    }

    return (
        <div className="settings-item" style={{ marginTop: 16 }}>
            <div className="settings-list">
                <DndContext
                    sensors={sensors}
                    collisionDetection={closestCenter}
                    onDragEnd={handleDragEnd}
                >
                    <SortableContext
                        items={displayIds}
                        strategy={verticalListSortingStrategy}
                    >
                        {displayIds.map(modelId => {
                            const model = PRESET_MODELS.find(m => m.id === modelId) || { id: modelId, name: modelId, description: '', filename: '', type: 'itn', engine: 'onnx', language: '', size: '', url: '' };
                            const isInstalled = installedITNModels.has(model.id);
                            const isEnabled = enabledITNModels.has(model.id);

                            return (
                                <SortableItem key={model.id} id={model.id} style={{ borderBottom: '1px solid var(--color-border-subtle)' }}>
                                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 0', width: '100%' }}>
                                        <div>
                                            <div style={{ fontWeight: 500 }}>{model.name}</div>
                                            <div style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}>{model.description}</div>
                                        </div>

                                        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                                            {!isInstalled ? (
                                                <>
                                                    {downloads[model.id] ? (
                                                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                                                            <span style={{ fontSize: '0.8rem' }}>{Math.round(downloads[model.id].progress)}%</span>
                                                            <button className="btn btn-sm btn-icon" onClick={() => onCancelDownload(model.id)} title="Cancel">
                                                                <XIcon />
                                                            </button>
                                                        </div>
                                                    ) : (
                                                        <button
                                                            className="btn btn-sm btn-secondary"
                                                            onClick={() => onDownload(model)}
                                                        // disabled={!!downloadingId} // Allow parallel
                                                        >
                                                            <DownloadIcon />
                                                            Download
                                                        </button>
                                                    )}
                                                </>
                                            ) : (
                                                <button
                                                    className="toggle-switch"
                                                    onClick={() => toggleModel(model.id)}
                                                    role="switch"
                                                    aria-checked={isEnabled}
                                                    aria-label={t('settings.toggle_model', { name: model.name })}
                                                    style={{ opacity: 1, cursor: 'pointer' }}
                                                    onPointerDown={(e) => e.stopPropagation()}
                                                >
                                                    <div className="toggle-switch-handle" />
                                                </button>
                                            )}
                                        </div>
                                    </div>
                                </SortableItem>
                            );
                        })}
                    </SortableContext>
                </DndContext>
            </div>
            <div className="settings-hint" style={{ marginTop: 8 }}>
                {t('settings.itn_note')}
            </div>
        </div>
    );
}
