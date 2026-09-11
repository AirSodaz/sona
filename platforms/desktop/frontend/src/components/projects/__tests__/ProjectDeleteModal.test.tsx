import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { ProjectDeleteModal } from '../ProjectDeleteModal';
import type { ProjectRecord } from '../../../types/project';
import i18n from '../../../i18n';

describe('ProjectDeleteModal', () => {
  const mockProject: ProjectRecord = {
    id: 'project-1',
    name: 'Marketing Campaign',
    description: 'Marketing audio files',
    icon: 'briefcase',
    color: '#6366f1',
    createdAt: 1000,
    updatedAt: 1000,
  };

  beforeEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('renders modal in English with items handling options', () => {
    render(
      <ProjectDeleteModal
        isOpen={true}
        project={mockProject}
        itemCount={5}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
      />
    );

    expect(screen.getByText('Delete Project')).toBeDefined();
    expect(screen.getByText('Are you sure you want to delete "Marketing Campaign"?')).toBeDefined();
    expect(screen.getByText('This project contains 5 items. Choose how to handle them:')).toBeDefined();
    expect(screen.getByText('Move to Inbox (Recommended)')).toBeDefined();
    expect(screen.getByText('Keep all recordings and transcriptions; reset their project to Inbox.')).toBeDefined();
    expect(screen.getByText('Move all items to Trash')).toBeDefined();
    expect(screen.getByText('Soft-delete all associated items and send them to Trash.')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDefined();
  });

  it('renders modal in Chinese (zh) with proper translations', async () => {
    await i18n.changeLanguage('zh');

    render(
      <ProjectDeleteModal
        isOpen={true}
        project={mockProject}
        itemCount={3}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
      />
    );

    expect(screen.getByText('删除项目')).toBeDefined();
    expect(screen.getByText('确定要删除项目“Marketing Campaign”吗？')).toBeDefined();
    expect(screen.getByText('该项目包含 3 条记录，请选择处理方式：')).toBeDefined();
    expect(screen.getByText('移至收件箱（推荐）')).toBeDefined();
    expect(screen.getByText('保留所有录音与转录文本，将归属项目重置为收件箱。')).toBeDefined();
    expect(screen.getByText('将所有记录移至回收站')).toBeDefined();
    expect(screen.getByText('软删除所有关联记录并放入回收站。')).toBeDefined();
    expect(screen.getByRole('button', { name: '取消' })).toBeDefined();
    expect(screen.getByRole('button', { name: '删除' })).toBeDefined();
  });

  it('renders empty project note when itemCount is 0 in Chinese', async () => {
    await i18n.changeLanguage('zh');

    render(
      <ProjectDeleteModal
        isOpen={true}
        project={mockProject}
        itemCount={0}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
      />
    );

    expect(screen.getByText('该项目下暂无记录，将被立即移除。')).toBeDefined();
    expect(screen.queryByText(/请选择处理方式/)).toBeNull();
  });

  it('allows choosing cascade action and invokes onConfirm', async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined);
    render(
      <ProjectDeleteModal
        isOpen={true}
        project={mockProject}
        itemCount={2}
        onClose={vi.fn()}
        onConfirm={onConfirm}
      />
    );

    const trashRadio = screen.getByDisplayValue('deleteItems');
    fireEvent.click(trashRadio);

    const deleteBtn = screen.getByRole('button', { name: 'Delete' });
    fireEvent.click(deleteBtn);

    expect(onConfirm).toHaveBeenCalledWith('deleteItems');
  });

  it('returns null when isOpen is false or project is null', () => {
    const { container: closedContainer } = render(
      <ProjectDeleteModal
        isOpen={false}
        project={mockProject}
        itemCount={2}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
      />
    );
    expect(closedContainer.innerHTML).toBe('');

    const { container: nullProjectContainer } = render(
      <ProjectDeleteModal
        isOpen={true}
        project={null}
        itemCount={2}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
      />
    );
    expect(nullProjectContainer.innerHTML).toBe('');
  });
});
