import type React from 'react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useSearchStore } from '../stores/searchStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { ChevronDownIcon, ChevronUpIcon, CloseIcon } from './Icons';
import { IconButton } from './ui/Button';
import { SearchInput } from './ui/SearchInput';

function getMatchCountText(matchCount: number, currentIndex: number, query: string): string {
  if (matchCount > 0) {
    return `${currentIndex + 1}/${matchCount}`;
  }
  if (query) {
    return '0/0';
  }
  return '';
}

export function SearchUI(): React.JSX.Element | null {
  const { t } = useTranslation();
  const {
    isOpen,
    query,
    matches,
    currentMatchIndex,
    close,
    setQuery,
    nextMatch,
    prevMatch,
    performSearch,
  } = useSearchStore();

  const segments = useTranscriptSessionStore((state) => state.segments);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Autofocus input when opened
  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.select();
    }
  }, [isOpen]);

  useEffect(() => {
    if (isOpen && typeof query === 'string') {
      performSearch(segments);
    }
  }, [isOpen, performSearch, query, segments]);

  useEscapeKey(
    (e) => {
      e.preventDefault();
      close();
    },
    {
      enabled: isOpen,
      checkTopMost: true,
      containerRef,
    }
  );

  if (!isOpen) return null;

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      if (e.shiftKey) {
        prevMatch();
      } else {
        nextMatch();
      }
    }
  };

  const matchCountText = getMatchCountText(matches.length, currentMatchIndex, query);

  return (
    <div
      ref={containerRef}
      className="search-ui-container"
      role="search"
      aria-label={t('search.label', 'Search transcript')}
    >
      <SearchInput
        ref={inputRef}
        wrapperClassName="search-bar"
        size="sm"
        type="text"
        placeholder={t('search.placeholder', 'Find in transcript...')}
        aria-label={t('search.placeholder', 'Find in transcript...')}
        clearLabel={t('common.clear_search', { defaultValue: 'Clear search' })}
        value={query}
        onValueChange={setQuery}
        onKeyDown={handleKeyDown}
        suffix={
          <div className="search-actions">
            <span className="search-count" aria-live="polite">
              {matchCountText}
            </span>

            <div className="search-divider" />

            <IconButton
              tier="ghost"
              size="xs"
              icon={<ChevronUpIcon />}
              onClick={prevMatch}
              data-tooltip={`${t('search.previous', 'Previous match')} (Shift+Enter)`}
              data-tooltip-pos="top"
              aria-label={t('search.previous', 'Previous match')}
            />
            <IconButton
              tier="ghost"
              size="xs"
              icon={<ChevronDownIcon />}
              onClick={nextMatch}
              data-tooltip={`${t('search.next', 'Next match')} (Enter)`}
              data-tooltip-pos="top"
              aria-label={t('search.next', 'Next match')}
            />
            <IconButton
              tier="ghost"
              size="xs"
              icon={<CloseIcon />}
              onClick={close}
              data-tooltip={`${t('search.close', 'Close')} (Esc)`}
              data-tooltip-pos="top"
              aria-label={t('search.close', 'Close')}
            />
          </div>
        }
      />
    </div>
  );
}
