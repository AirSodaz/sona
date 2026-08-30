import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { LanguageBadges } from '../LanguageBadges';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { sample?: string; count?: number; defaultValue?: string }) => {
      if (key === 'languages.tooltip_summary' && options?.sample && options?.count) {
        return `${options.sample} 等共 ${options.count} 种语言`;
      }
      return options?.defaultValue ?? key;
    },
    i18n: { language: 'zh' },
  }),
}));

describe('LanguageBadges', () => {
  it('renders nothing when languages array is empty or undefined', () => {
    const { container: c1 } = render(<LanguageBadges languages={undefined} />);
    expect(c1.firstChild).toBeNull();

    const { container: c2 } = render(<LanguageBadges languages={[]} />);
    expect(c2.firstChild).toBeNull();
  });

  it('renders visible language badges and applies custom data-tooltip without native title', () => {
    const { container } = render(<LanguageBadges languages={['zh', 'en']} />);

    const cluster = container.querySelector('.language-badges');
    expect(cluster).not.toBeNull();
    expect(cluster?.getAttribute('title')).toBeNull();
    expect(cluster?.getAttribute('data-tooltip')).toBe('中文, 英语');
    expect(cluster?.getAttribute('data-tooltip-pos')).toBe('top');
    expect(cluster?.hasAttribute('data-tooltip-multiline')).toBe(true);

    expect(screen.getByText('ZH')).toBeDefined();
    expect(screen.getByText('EN')).toBeDefined();
    expect(container.querySelector('.model-tag-overflow')).toBeNull();
  });

  it('renders overflow chip and summarized tooltip for long language list', () => {
    const longLanguages = [
      'af', 'am', 'ar', 'as', 'az', 'ba', 'be', 'bg', 'bn', 'bo',
      'br', 'bs', 'ca', 'cs', 'cy', 'da', 'de', 'el', 'en', 'es',
      'et', 'eu', 'fa', 'fi', 'fil', 'fo', 'fr', 'gl', 'gu', 'ha',
    ];
    const { container } = render(<LanguageBadges languages={longLanguages} />);

    const cluster = container.querySelector('.language-badges');
    expect(cluster).not.toBeNull();
    expect(cluster?.getAttribute('title')).toBeNull();

    const tooltip = cluster?.getAttribute('data-tooltip') ?? '';
    expect(tooltip).toContain('等共 30 种语言');

    // 4 visible + 26 overflow
    expect(screen.getByText('AF')).toBeDefined();
    expect(screen.getByText('AM')).toBeDefined();
    expect(screen.getByText('AR')).toBeDefined();
    expect(screen.getByText('AS')).toBeDefined();
    expect(screen.getByText('+26')).toBeDefined();
  });

  it('provides accessible role and keyboard focusability', () => {
    const { container } = render(<LanguageBadges languages={['zh']} />);
    const cluster = container.querySelector('.language-badges');

    expect(cluster?.getAttribute('role')).toBe('note');
    expect(cluster?.getAttribute('tabIndex')).toBe('0');
    expect(cluster?.getAttribute('aria-label')).toBe('中文');
  });
});
