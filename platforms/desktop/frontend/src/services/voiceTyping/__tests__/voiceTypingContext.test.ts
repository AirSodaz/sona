import { describe, expect, it } from 'vitest';
import type { VoiceTypingContextRule } from '../../../types/config';
import {
  classifyContextMode,
  classifyContextRule,
  DEFAULT_VOICE_TYPING_CONTEXT_RULES,
  getContextDirective,
  matchContextRule,
  shouldStripTrailingPunctuation,
} from '../voiceTypingContext';

describe('voiceTypingContext', () => {
  describe('per-platform rule matching', () => {
    it('matches Windows apps strictly when platform is windows', () => {
      const devRule = DEFAULT_VOICE_TYPING_CONTEXT_RULES.find((r) => r.id === 'developer')!;
      expect(matchContextRule(devRule, 'code.exe', '', 'windows')).toBe(true);
      expect(matchContextRule(devRule, 'Visual Studio Code', '', 'windows')).toBe(false);
      expect(matchContextRule(devRule, 'devenv.exe', '', 'windows')).toBe(true);
    });

    it('matches macOS apps strictly when platform is macos', () => {
      const devRule = DEFAULT_VOICE_TYPING_CONTEXT_RULES.find((r) => r.id === 'developer')!;
      expect(matchContextRule(devRule, 'Visual Studio Code', '', 'macos')).toBe(true);
      expect(matchContextRule(devRule, 'Xcode', '', 'macos')).toBe(true);
      expect(matchContextRule(devRule, 'code.exe', '', 'macos')).toBe(false);
    });

    it('matches Linux apps strictly when platform is linux', () => {
      const devRule = DEFAULT_VOICE_TYPING_CONTEXT_RULES.find((r) => r.id === 'developer')!;
      expect(matchContextRule(devRule, 'code', '', 'linux')).toBe(true);
      expect(matchContextRule(devRule, 'gnome-terminal', '', 'linux')).toBe(true);
      expect(matchContextRule(devRule, 'devenv.exe', '', 'linux')).toBe(false);
    });

    it('matches cross-platform window title patterns across any platform', () => {
      const devRule = DEFAULT_VOICE_TYPING_CONTEXT_RULES.find((r) => r.id === 'developer')!;
      expect(matchContextRule(devRule, 'chrome.exe', 'PR #42 - GitHub', 'windows')).toBe(true);
      expect(matchContextRule(devRule, 'Google Chrome', 'PR #42 - GitHub', 'macos')).toBe(true);
      expect(matchContextRule(devRule, 'chromium', 'PR #42 - GitHub', 'linux')).toBe(true);
    });
  });

  describe('custom context rules', () => {
    const customRules: VoiceTypingContextRule[] = [
      ...DEFAULT_VOICE_TYPING_CONTEXT_RULES,
      {
        id: 'academic',
        name: 'Academic Paper',
        icon: '🎓',
        appsByPlatform: {
          windows: ['zotero.exe', 'overleaf.exe'],
          macos: ['Zotero', 'TeXShop'],
          linux: ['zotero', 'kile'],
        },
        titlePatterns: ['overleaf.com', 'arxiv.org'],
        promptDirective: 'Academic Mode: Use rigorous scholarly language and passive voice.',
        stripTrailingPunctuation: false,
        isBuiltin: false,
        enabled: true,
      },
    ];

    it('matches custom rule by platform app', () => {
      const rule = classifyContextRule('zotero.exe', '', customRules, 'windows');
      expect(rule).toBeDefined();
      expect(rule?.id).toBe('academic');
      expect(rule?.name).toBe('Academic Paper');
      expect(rule?.icon).toBe('🎓');
    });

    it('matches custom rule by window title keyword', () => {
      const rule = classifyContextRule(
        'chrome.exe',
        'Draft on overleaf.com',
        customRules,
        'windows'
      );
      expect(rule?.id).toBe('academic');
    });

    it('generates directive from custom rule', () => {
      const rule = customRules.find((r) => r.id === 'academic')!;
      const directive = getContextDirective(rule, 'Introduction Chapter');
      expect(directive).toContain('Academic Mode:');
      expect(directive).toContain('Introduction Chapter');
    });

    it('respects stripTrailingPunctuation flag on custom rule', () => {
      const chatRule = customRules.find((r) => r.id === 'chat')!;
      const academicRule = customRules.find((r) => r.id === 'academic')!;
      expect(shouldStripTrailingPunctuation(chatRule)).toBe(true);
      expect(shouldStripTrailingPunctuation(academicRule)).toBe(false);
    });

    it('respects rule.enabled toggle', () => {
      const disabledRules = customRules.map((r) =>
        r.id === 'academic' ? { ...r, enabled: false } : r
      );
      const rule = classifyContextRule('zotero.exe', '', disabledRules, 'windows');
      expect(rule).toBeNull();
    });
  });

  describe('classifyContextMode backwards compatibility', () => {
    it('classifies apps with default rules and platform', () => {
      expect(
        classifyContextMode('code.exe', '', 'auto', DEFAULT_VOICE_TYPING_CONTEXT_RULES, 'windows')
      ).toBe('developer');
      expect(
        classifyContextMode('slack.exe', '', 'auto', DEFAULT_VOICE_TYPING_CONTEXT_RULES, 'windows')
      ).toBe('chat');
      expect(
        classifyContextMode(
          'winword.exe',
          '',
          'auto',
          DEFAULT_VOICE_TYPING_CONTEXT_RULES,
          'windows'
        )
      ).toBe('formal');
      expect(
        classifyContextMode(
          'unknown.exe',
          '',
          'auto',
          DEFAULT_VOICE_TYPING_CONTEXT_RULES,
          'windows'
        )
      ).toBe('general');
    });

    it('respects manual preset lock', () => {
      expect(
        classifyContextMode('code.exe', '', 'chat', DEFAULT_VOICE_TYPING_CONTEXT_RULES, 'windows')
      ).toBe('chat');
      expect(
        classifyContextMode(
          'wechat.exe',
          '',
          'formal',
          DEFAULT_VOICE_TYPING_CONTEXT_RULES,
          'windows'
        )
      ).toBe('formal');
      expect(
        classifyContextMode(
          'code.exe',
          '',
          'general',
          DEFAULT_VOICE_TYPING_CONTEXT_RULES,
          'windows'
        )
      ).toBe('general');
    });
    it('matches Windows apps with or without .exe suffix seamlessly', () => {
      expect(
        classifyContextMode('code', '', 'auto', DEFAULT_VOICE_TYPING_CONTEXT_RULES, 'windows')
      ).toBe('developer');
      const customRule: VoiceTypingContextRule = {
        id: 'team_chat',
        name: 'Team',
        appsByPlatform: { windows: ['slack'], macos: [], linux: [] },
        titlePatterns: [],
        promptDirective: '',
        stripTrailingPunctuation: true,
        isBuiltin: false,
        enabled: true,
      };
      expect(classifyContextMode('slack.exe', '', 'auto', [customRule], 'windows')).toBe(
        'team_chat'
      );
    });

    it('truncates window title to 100 chars and removes newlines in directive', () => {
      const longTitle = 'a'.repeat(150) + '\nwith\tnewlines';
      const directive = getContextDirective('developer', longTitle);
      expect(directive).toContain('Active window context and topic: "');
      expect(directive).not.toContain('\nwith\tnewlines');
      const match = directive.match(/Active window context and topic: "([^"]+)"/);
      expect(match?.[1].length).toBe(100);
    });

    it('handles missing or undefined appsByPlatform defensively', () => {
      const malformedRule = {
        id: 'test',
        name: 'Test',
        titlePatterns: ['pattern'],
        promptDirective: '',
        stripTrailingPunctuation: false,
        isBuiltin: false,
        enabled: true,
      } as unknown as VoiceTypingContextRule;
      expect(() => matchContextRule(malformedRule, 'app.exe', '', 'windows')).not.toThrow();
      expect(matchContextRule(malformedRule, 'app.exe', '', 'windows')).toBe(false);
      expect(matchContextRule(malformedRule, 'app.exe', 'pattern', 'windows')).toBe(true);
    });
  });
});
