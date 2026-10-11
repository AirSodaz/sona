import type React from 'react';
import type { HistoryItem as HistoryItemType } from '../../types/history';
import type { ProjectRecord } from '../../types/project';
import { RenameModal } from '../RenameModal';
import { ProjectAssignmentModal } from './ProjectAssignmentModal';
import { ProjectCreateModal } from './ProjectCreateModal';
import { ProjectDeleteModal } from './ProjectDeleteModal';
import { ProjectSettingsModal } from './ProjectSettingsModal';
import type { RenameTarget } from './types';

export interface ProjectsModalsProps {
  // Create Modal
  isCreateModalOpen: boolean;
  newProjectName: string;
  newProjectDescription: string;
  newProjectColor: string;
  newProjectIcon: string;
  onNewProjectNameChange: (name: string) => void;
  onNewProjectDescriptionChange: (desc: string) => void;
  onNewProjectColorChange: (color: string) => void;
  onNewProjectIconChange: (icon: string) => void;
  onCloseCreateModal: () => void;
  onCreateProject: () => void;

  // Settings Modal
  isSettingsOpen: boolean;
  browseProject: ProjectRecord | null;
  draftName: string;
  draftDescription: string;
  draftIcon: string;
  draftColor: string;
  draftPipeline?: ProjectRecord['pipeline'];
  onDraftPipelineChange?: (pipeline: ProjectRecord['pipeline']) => void;
  onCloseSettings: () => void;
  onSaveProject: () => void;
  onDeleteProject: () => void;
  onDraftNameChange: (name: string) => void;
  onDraftDescriptionChange: (desc: string) => void;
  onDraftIconChange: (icon: string) => void;
  onDraftColorChange: (color: string) => void;

  // Rename Modal
  renameTarget: RenameTarget | null;
  onCloseRenameModal: () => void;
  onPerformRename: (title: string, icon?: string) => Promise<void>;
  onAiAction: () => Promise<string>;

  // Assignment Modal
  isAssignmentOpen: boolean;
  assignmentItems: HistoryItemType[];
  projects: ProjectRecord[];
  onCloseAssignmentModal: () => void;
  onApplyAssignment: (projectId: string | null) => Promise<void>;

  // Delete Modal
  projectToDelete: ProjectRecord | null;
  deleteItemCount: number;
  onCloseDeleteModal: () => void;
  onConfirmDelete: (cascadeAction: 'moveToInbox' | 'deleteItems') => Promise<void>;
}

export function ProjectsModals({
  isCreateModalOpen,
  newProjectName,
  newProjectDescription,
  newProjectColor,
  newProjectIcon,
  onNewProjectNameChange,
  onNewProjectDescriptionChange,
  onNewProjectColorChange,
  onNewProjectIconChange,
  onCloseCreateModal,
  onCreateProject,
  isSettingsOpen,
  browseProject,
  draftName,
  draftDescription,
  draftIcon,
  draftColor,
  draftPipeline,
  onDraftPipelineChange,
  onCloseSettings,
  onSaveProject,
  onDeleteProject,
  onDraftNameChange,
  onDraftDescriptionChange,
  onDraftIconChange,
  onDraftColorChange,
  renameTarget,
  onCloseRenameModal,
  onPerformRename,
  onAiAction,
  isAssignmentOpen,
  assignmentItems,
  projects,
  onCloseAssignmentModal,
  onApplyAssignment,
  projectToDelete,
  deleteItemCount,
  onCloseDeleteModal,
  onConfirmDelete,
}: ProjectsModalsProps): React.JSX.Element {
  return (
    <>
      <ProjectCreateModal
        isOpen={isCreateModalOpen}
        name={newProjectName}
        description={newProjectDescription}
        color={newProjectColor}
        icon={newProjectIcon}
        onNameChange={onNewProjectNameChange}
        onDescriptionChange={onNewProjectDescriptionChange}
        onColorChange={onNewProjectColorChange}
        onIconChange={onNewProjectIconChange}
        onClose={onCloseCreateModal}
        onCreate={onCreateProject}
      />

      <ProjectSettingsModal
        isOpen={isSettingsOpen}
        project={browseProject}
        draftName={draftName}
        draftDescription={draftDescription}
        draftIcon={draftIcon}
        draftColor={draftColor}
        draftPipeline={draftPipeline}
        onPipelineChange={onDraftPipelineChange}
        onClose={onCloseSettings}
        onSave={onSaveProject}
        onDelete={onDeleteProject}
        onNameChange={onDraftNameChange}
        onDescriptionChange={onDraftDescriptionChange}
        onIconChange={onDraftIconChange}
        onColorChange={onDraftColorChange}
      />

      <RenameModal
        isOpen={!!renameTarget}
        onClose={onCloseRenameModal}
        initialTitle={renameTarget?.title || ''}
        initialIcon={renameTarget?.icon}
        defaultType={renameTarget?.type}
        onRename={onPerformRename}
        onAiAction={onAiAction}
      />

      <ProjectAssignmentModal
        isOpen={isAssignmentOpen}
        items={assignmentItems}
        projects={projects}
        onClose={onCloseAssignmentModal}
        onApply={onApplyAssignment}
      />

      <ProjectDeleteModal
        isOpen={!!projectToDelete}
        project={projectToDelete}
        itemCount={deleteItemCount}
        onClose={onCloseDeleteModal}
        onConfirm={onConfirmDelete}
      />
    </>
  );
}
