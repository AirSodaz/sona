import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useSearchStore } from '../stores/searchStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { ChevronUpIcon, ChevronDownIcon, CloseIcon } from './Icons';

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
        performSearch
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
        if (isOpen) {
            performSearch(segments);
        }
    }, [isOpen, performSearch, query, segments]);

    useEscapeKey((e) => {
        e.preventDefault();
        close();
    }, {
        enabled: isOpen,
        checkTopMost: true,
        containerRef,
    });

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
        <div ref={containerRef} className="search-ui-container" role="search" aria-label={t('search.label', 'Search transcript')}>
            <div className="search-bar">
                <input
                    ref={inputRef}
                    type="text"
                    className="search-input"
                    placeholder={t('search.placeholder', 'Find in transcript...')}
                    aria-label={t('search.placeholder', 'Find in transcript...')}
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    onKeyDown={handleKeyDown}
                />

                <div className="search-actions">
                    <span className="search-count" aria-live="polite">
                        {matchCountText}
                    </span>

                    <div className="search-divider" />

                    <button
                        className="btn-icon-sm"
                        onClick={prevMatch}
                        data-tooltip={`${t('search.previous', 'Previous match')} (Shift+Enter)`}
                        data-tooltip-pos="top"
                        aria-label={t('search.previous', 'Previous match')}
                    >
                        <ChevronUpIcon />
                    </button>
                    <button
                        className="btn-icon-sm"
                        onClick={nextMatch}
                        data-tooltip={`${t('search.next', 'Next match')} (Enter)`}
                        data-tooltip-pos="top"
                        aria-label={t('search.next', 'Next match')}
                    >
                        <ChevronDownIcon />
                    </button>
                    <button
                        className="btn-icon-sm"
                        onClick={close}
                        data-tooltip={`${t('search.close', 'Close')} (Esc)`}
                        data-tooltip-pos="top"
                        aria-label={t('search.close', 'Close')}
                    >
                        <CloseIcon />
                    </button>
                </div>
            </div>
        </div>
    );
}
