import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { ProjectSettingsModal } from '../ProjectSettingsModal';
import type { ProjectRecord } from '../../../types/project';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    i18n: { language: 'en' },
  }),
}));

vi.mock('../../../services/tauri/platform/dialog', () => ({
  openDialog: vi.fn(),
}));

describe('ProjectSettingsModal', () => {
  const mockProject: ProjectRecord = {
    id: 'project-1',
    name: 'Test Project',
    description: 'Test description',
    icon: 'folder',
    color: '#6366f1',
    createdAt: 1000,
    updatedAt: 1000,
    pipeline: {
      enabled: true,
      autoPolish: true,
      polishPresetId: 'general',
      autoTranslate: true,
      targetLanguage: 'en',
      autoSummary: true,
      summaryTemplateId: 'general',
      autoExport: true,
      exportFormat: 'txt',
      exportDirectory: '/tmp/export',
    },
  };

  it('renders pipeline dropdowns with design system Dropdown component', () => {
    const onPipelineChange = vi.fn();
    render(
      <ProjectSettingsModal
        isOpen={true}
        project={mockProject}
        draftName="Test Project"
        draftDescription="Test description"
        draftIcon="folder"
        draftColor="#6366f1"
        draftPipeline={mockProject.pipeline}
        onClose={vi.fn()}
        onSave={vi.fn()}
        onDelete={vi.fn()}
        onNameChange={vi.fn()}
        onDescriptionChange={vi.fn()}
        onIconChange={vi.fn()}
        onColorChange={vi.fn()}
        onPipelineChange={onPipelineChange}
      />
    );

    // Verify each pipeline setting dropdown button exists (via Dropdown aria-label)
    expect(screen.getByRole('button', { name: 'Auto Polish' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Auto Translate' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Auto Summary' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Export Format' })).toBeDefined();
  });

  it('triggers onPipelineChange when changing auto-export format', () => {
    const onPipelineChange = vi.fn();
    render(
      <ProjectSettingsModal
        isOpen={true}
        project={mockProject}
        draftName="Test Project"
        draftDescription="Test description"
        draftIcon="folder"
        draftColor="#6366f1"
        draftPipeline={mockProject.pipeline}
        onClose={vi.fn()}
        onSave={vi.fn()}
        onDelete={vi.fn()}
        onNameChange={vi.fn()}
        onDescriptionChange={vi.fn()}
        onIconChange={vi.fn()}
        onColorChange={vi.fn()}
        onPipelineChange={onPipelineChange}
      />
    );

    // Click the Export Format dropdown trigger button
    const formatTrigger = screen.getByRole('button', { name: 'Export Format' });
    fireEvent.click(formatTrigger);

    // Click the SRT option
    const srtOption = screen.getByRole('option', { name: 'SRT' });
    fireEvent.click(srtOption);

    expect(onPipelineChange).toHaveBeenCalledWith(
      expect.objectContaining({
        exportFormat: 'srt',
      })
    );
  });

  it('renders pipeline features as switches and toggles them', () => {
    const onPipelineChange = vi.fn();
    render(
      <ProjectSettingsModal
        isOpen={true}
        project={mockProject}
        draftName="Test Project"
        draftDescription="Test description"
        draftIcon="folder"
        draftColor="#6366f1"
        draftPipeline={mockProject.pipeline}
        onClose={vi.fn()}
        onSave={vi.fn()}
        onDelete={vi.fn()}
        onNameChange={vi.fn()}
        onDescriptionChange={vi.fn()}
        onIconChange={vi.fn()}
        onColorChange={vi.fn()}
        onPipelineChange={onPipelineChange}
      />
    );

    // Verify pipeline enable and feature switches exist
    expect(screen.getByRole('switch', { name: 'Enable Project Pipeline' })).toBeDefined();
    const polishSwitch = screen.getByRole('switch', { name: 'Auto Polish' });
    expect(polishSwitch).toBeDefined();
    expect(screen.getByRole('switch', { name: 'Auto Translate' })).toBeDefined();
    expect(screen.getByRole('switch', { name: 'Auto Summary' })).toBeDefined();
    expect(screen.getByRole('switch', { name: 'Auto Export' })).toBeDefined();

    // Toggle Auto Polish switch
    fireEvent.click(polishSwitch);
    expect(onPipelineChange).toHaveBeenCalledWith(
      expect.objectContaining({
        autoPolish: false,
      })
    );
  });
});
