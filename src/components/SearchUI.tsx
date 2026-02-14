import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useSearchStore } from '../stores/searchStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { ChevronUpIcon, ChevronDownIcon, CloseIcon } from './Icons';

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

    const segments = useTranscriptStore(state => state.segments);
    const inputRef = useRef<HTMLInputElement>(null);

    // Autofocus input when opened
    useEffect(() => {
        if (isOpen && inputRef.current) {
            inputRef.current.select();
        }
    }, [isOpen]);

    // Re-run search when segments change if open
    useEffect(() => {
        if (isOpen && query) {
            performSearch(segments);
        }
    }, [segments, isOpen, performSearch]); // query is excluded to avoid loop if performSearch updates it (it doesn't, but still)

    // Run search when query changes
    useEffect(() => {
        if (isOpen) {
            performSearch(segments);
        }
    }, [query, isOpen, performSearch, segments]);

    if (!isOpen) return null;

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') {
            if (e.shiftKey) {
                prevMatch();
            } else {
                nextMatch();
            }
        } else if (e.key === 'Escape') {
            close();
        }
    };

    return (
        <div className="search-ui-container">
            <div className="search-bar">
                <input
                    ref={inputRef}
                    type="text"
                    className="search-input"
                    placeholder={t('search.placeholder', 'Find in transcript...')}
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    onKeyDown={handleKeyDown}
                />

                <div className="search-actions">
                    <span className="search-count">
                        {matches.length > 0 ? `${currentMatchIndex + 1}/${matches.length}` : (query ? '0/0' : '')}
                    </span>

                    <div className="search-divider" />

                    <button className="btn-icon-sm" onClick={prevMatch} title={t('search.previous', 'Previous match')}>
                        <ChevronUpIcon />
                    </button>
                    <button className="btn-icon-sm" onClick={nextMatch} title={t('search.next', 'Next match')}>
                        <ChevronDownIcon />
                    </button>
                    <button className="btn-icon-sm" onClick={close} title={t('search.close', 'Close')}>
                        <CloseIcon />
                    </button>
                </div>
            </div>

            <style>{`
                .search-ui-container {
                    position: absolute;
                    bottom: 24px;
                    left: 50%;
                    transform: translateX(-50%);
                    z-index: 100;
                    display: flex;
                    flex-direction: column;
                    gap: 8px;
                    animation: slideUp 0.2s cubic-bezier(0.16, 1, 0.3, 1);
                }

                .search-bar {
                    display: flex;
                    align-items: center;
                    background: var(--color-bg-elevated);
                    border: 1px solid var(--color-border);
                    border-radius: var(--radius-lg);
                    padding: 4px;
                    box-shadow: var(--shadow-xl);
                    min-width: 320px;
                }

                .search-input {
                    border: none;
                    background: transparent;
                    padding: 6px 12px;
                    font-size: 14px;
                    width: 100%;
                    color: var(--color-text-primary);
                    outline: none;
                }
                
                .search-input:focus {
                    box-shadow: none;
                    background: transparent;
                }

                .search-actions {
                    display: flex;
                    align-items: center;
                    gap: 2px;
                    padding-right: 4px;
                    flex-shrink: 0;
                }

                .search-count {
                    font-size: 12px;
                    color: var(--color-text-muted);
                    padding: 0 8px;
                    font-feature-settings: "tnum";
                    white-space: nowrap;
                }

                .search-divider {
                    width: 1px;
                    height: 16px;
                    background: var(--color-border);
                    margin: 0 4px;
                }

                .btn-icon-sm {
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    width: 24px;
                    height: 24px;
                    border: none;
                    background: transparent;
                    color: var(--color-text-secondary);
                    border-radius: 4px;
                    cursor: pointer;
                    transition: all 0.15s ease;
                }

                .btn-icon-sm:hover {
                    background: var(--color-bg-hover);
                    color: var(--color-text-primary);
                }
                
                .btn-icon-sm svg {
                    width: 16px;
                    height: 16px;
                }

                @keyframes slideUp {
                    from { opacity: 0; transform: translate(-50%, 10px); }
                    to { opacity: 1; transform: translate(-50%, 0); }
                }
            `}</style>
        </div>
    );
}
