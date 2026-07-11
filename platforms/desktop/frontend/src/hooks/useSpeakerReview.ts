import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useDialogStore } from '../stores/dialogStore';
import { useConfigStore } from '../stores/configStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import {
  buildSpeakerCorrectionProfileSections,
  speakerCorrectionService,
} from '../services/speakerCorrectionService';
import {
  buildSpeakerReviewSnapshot,
  type SpeakerReviewCounts,
  type SpeakerReviewFilter,
  type SpeakerReviewGroup,
  type SpeakerReviewSnapshot,
} from '../services/speakerReviewService';

const EMPTY_SPEAKER_REVIEW_COUNTS: SpeakerReviewCounts = {
  total: 0,
  pending: 0,
  suggested: 0,
  anonymous: 0,
  identified: 0,
  reviewed: 0,
};

function createEmptySpeakerReviewSnapshot(): SpeakerReviewSnapshot {
  return {
    groups: [],
    counts: EMPTY_SPEAKER_REVIEW_COUNTS,
    visibleGroups: [],
    filterOptions: [],
  };
}

function resolveNextActiveGroupId(groupId: string, groups: SpeakerReviewGroup[]): string | null {
  if (groups.length === 0) {
    return null;
  }

  const currentIndex = groups.findIndex((group) => group.groupId === groupId);
  if (currentIndex < 0) {
    return groups[0]?.groupId || null;
  }

  return groups[currentIndex + 1]?.groupId
    || groups[currentIndex - 1]?.groupId
    || null;
}

function isShortcutIgnoredTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }

  return Boolean(target.closest('input, textarea, select, button, [contenteditable="true"]'));
}

export interface UseSpeakerReviewOptions {
  isOpen: boolean;
  onClose: () => void;
  modalRef: React.RefObject<HTMLDivElement | null>;
}

export function useSpeakerReview({ isOpen, onClose, modalRef }: UseSpeakerReviewOptions) {
  const busyGroupIdRef = useRef<string | null>(null);
  const snapshotRequestIdRef = useRef(0);
  const showError = useDialogStore((state) => state.showError);
  const segments = useTranscriptSessionStore((state) => state.segments);
  const speakerProfiles = useConfigStore((state) => state.config.speakerProfiles);
  const requestSeek = useTranscriptPlaybackStore((state) => state.requestSeek);
  const activeProject = useProjectStore((state) => (
    state.activeProjectId
      ? state.projects.find((project) => project.id === state.activeProjectId) || null
      : null
  ));
  
  const profileSections = useMemo(
    () => buildSpeakerCorrectionProfileSections(speakerProfiles, activeProject),
    [activeProject, speakerProfiles],
  );
  
  const [activeFilter, setActiveFilter] = useState<SpeakerReviewFilter>('pending');
  const [snapshot, setSnapshot] = useState<SpeakerReviewSnapshot>(() => createEmptySpeakerReviewSnapshot());
  const [isSnapshotLoading, setIsSnapshotLoading] = useState(false);
  const [expandedGroupIds, setExpandedGroupIds] = useState<Set<string>>(new Set());
  const [busyGroupId, setBusyGroupId] = useState<string | null>(null);
  const [activeGroupId, setActiveGroupId] = useState<string | null>(null);
  
  const visibleGroups = snapshot.visibleGroups;
  const effectiveActiveGroupId = (
    activeGroupId && visibleGroups.some((group) => group.groupId === activeGroupId)
      ? activeGroupId
      : visibleGroups[0]?.groupId || null
  );
  
  const activeGroup = useMemo(
    () => visibleGroups.find((group) => group.groupId === effectiveActiveGroupId) || null,
    [effectiveActiveGroupId, visibleGroups],
  );

  useEffect(() => {
    if (!isOpen) {
      snapshotRequestIdRef.current += 1;
      return undefined;
    }

    const requestId = snapshotRequestIdRef.current + 1;
    snapshotRequestIdRef.current = requestId;
    let cancelled = false;

    void Promise.resolve()
      .then(() => {
        if (!cancelled && snapshotRequestIdRef.current === requestId) {
          setIsSnapshotLoading(true);
        }
        return buildSpeakerReviewSnapshot(segments, activeFilter);
      })
      .then((nextSnapshot) => {
        if (cancelled || snapshotRequestIdRef.current !== requestId) {
          return;
        }

        setSnapshot(nextSnapshot);
        setActiveGroupId((current) => (
          current && nextSnapshot.visibleGroups.some((group) => group.groupId === current)
            ? current
            : nextSnapshot.visibleGroups[0]?.groupId || null
        ));
      })
      .catch(async (error) => {
        if (cancelled || snapshotRequestIdRef.current !== requestId) {
          return;
        }

        setSnapshot(createEmptySpeakerReviewSnapshot());
        await showError({
          code: 'speaker_review.snapshot_failed',
          messageKey: 'editor.speaker_correction_failed',
          cause: error,
        });
      })
      .finally(() => {
        if (!cancelled && snapshotRequestIdRef.current === requestId) {
          setIsSnapshotLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [activeFilter, isOpen, segments, showError]);

  const runGroupAction = useCallback(async (
    groupId: string,
    action: () => Promise<unknown>,
    errorCode: string,
  ) => {
    if (busyGroupIdRef.current) {
      return;
    }

    try {
      busyGroupIdRef.current = groupId;
      setBusyGroupId(groupId);
      await action();
      setActiveGroupId(resolveNextActiveGroupId(groupId, visibleGroups));
    } catch (error) {
      await showError({
        code: errorCode,
        messageKey: 'editor.speaker_correction_failed',
        cause: error,
      });
    } finally {
      busyGroupIdRef.current = null;
      setBusyGroupId(null);
    }
  }, [showError, visibleGroups]);

  const handleConfirmGroup = useCallback(async (groupId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.confirmSpeakerGroupReview(groupId),
      'speaker_review.confirm_failed',
    );
  }, [runGroupAction]);

  const handleAssignProfile = useCallback(async (groupId: string, profileId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.assignProfileToSpeakerGroup(groupId, profileId),
      'speaker_review.apply_failed',
    );
  }, [runGroupAction]);

  const handleResetGroup = useCallback(async (groupId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.resetGroupToAnonymous(groupId),
      'speaker_review.reset_failed',
    );
  }, [runGroupAction]);

  const handleJumpToGroup = useCallback((group: SpeakerReviewGroup) => {
    requestSeek(group.firstStart);
    onClose();
  }, [onClose, requestSeek]);

  const moveActiveGroup = useCallback((direction: 1 | -1) => {
    setActiveGroupId((current) => {
      if (visibleGroups.length === 0) {
        return null;
      }

      const currentIndex = current
        ? visibleGroups.findIndex((group) => group.groupId === current)
        : -1;
      if (currentIndex < 0) {
        return direction > 0
          ? visibleGroups[0].groupId
          : visibleGroups[visibleGroups.length - 1].groupId;
      }

      const nextIndex = Math.min(
        visibleGroups.length - 1,
        Math.max(0, currentIndex + direction),
      );
      return visibleGroups[nextIndex].groupId;
    });
  }, [visibleGroups]);

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.target instanceof Node
        && modalRef.current
        && !modalRef.current.contains(event.target)
      ) {
        return;
      }

      if (event.defaultPrevented || isShortcutIgnoredTarget(event.target)) {
        return;
      }

      const key = event.key.toLowerCase();
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        moveActiveGroup(1);
        return;
      }

      if (event.key === 'ArrowUp') {
        event.preventDefault();
        moveActiveGroup(-1);
        return;
      }

      if (!activeGroup || busyGroupIdRef.current) {
        return;
      }

      if (event.key === 'Enter') {
        event.preventDefault();
        void handleConfirmGroup(activeGroup.groupId);
        return;
      }

      if (key === 'a' && activeGroup.candidates[0]) {
        event.preventDefault();
        void handleAssignProfile(activeGroup.groupId, activeGroup.candidates[0].profileId);
        return;
      }

      if (key === 'r' && activeGroup.state !== 'anonymous') {
        event.preventDefault();
        void handleResetGroup(activeGroup.groupId);
        return;
      }

      if (key === 'j') {
        event.preventDefault();
        handleJumpToGroup(activeGroup);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    activeGroup,
    handleAssignProfile,
    handleConfirmGroup,
    handleJumpToGroup,
    handleResetGroup,
    isOpen,
    modalRef,
    moveActiveGroup,
  ]);

  const toggleExpanded = useCallback((groupId: string) => {
    setExpandedGroupIds((current) => {
      const next = new Set(current);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  }, []);

  return {
    snapshot,
    activeFilter,
    isSnapshotLoading,
    expandedGroupIds,
    busyGroupId,
    activeGroupId,
    effectiveActiveGroupId,
    profileSections,
    setActiveFilter,
    setActiveGroupId,
    toggleExpanded,
    handleConfirmGroup,
    handleAssignProfile,
    handleResetGroup,
    handleJumpToGroup,
  };
}
