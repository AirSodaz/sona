import { describe, expect, it } from 'vitest';
import { classifyContextMode, getContextDirective } from '../voiceTypingContext';

describe('voiceTypingContext', () => {
  describe('classifyContextMode', () => {
    it('classifies IDEs and terminal emulators as developer mode in auto preset', () => {
      expect(classifyContextMode('code.exe', 'index.ts - project')).toBe('developer');
      expect(classifyContextMode('devenv.exe', 'Solution')).toBe('developer');
      expect(classifyContextMode('windowsterminal.exe', 'cmd.exe')).toBe('developer');
      expect(classifyContextMode('zed.exe', 'main.rs')).toBe('developer');
    });

    it('classifies chat and IM apps as chat mode in auto preset', () => {
      expect(classifyContextMode('wechat.exe', 'WeChat')).toBe('chat');
      expect(classifyContextMode('slack.exe', 'General - Sona')).toBe('chat');
      expect(classifyContextMode('discord.exe', '#announcements')).toBe('chat');
      expect(classifyContextMode('feishu.exe', 'Feishu')).toBe('chat');
    });

    it('classifies office and notes apps as formal mode in auto preset', () => {
      expect(classifyContextMode('winword.exe', 'Document1 - Word')).toBe('formal');
      expect(classifyContextMode('wps.exe', 'Annual_Report.docx')).toBe('formal');
      expect(classifyContextMode('outlook.exe', 'Inbox - Outlook')).toBe('formal');
      expect(classifyContextMode('notion.exe', 'Sprint Planning')).toBe('formal');
    });

    it('uses window title heuristics when app name is generic browser', () => {
      expect(classifyContextMode('msedge.exe', 'PR #42: Refactor auth · GitHub')).toBe('developer');
      expect(classifyContextMode('chrome.exe', 'Slack | Channel 1')).toBe('chat');
      expect(classifyContextMode('chrome.exe', 'Random Web Page')).toBe('general');
    });

    it('respects explicit manual presets regardless of detected app', () => {
      expect(classifyContextMode('code.exe', 'editor', 'chat')).toBe('chat');
      expect(classifyContextMode('wechat.exe', 'chat', 'formal')).toBe('formal');
      expect(classifyContextMode('winword.exe', 'doc', 'developer')).toBe('developer');
      expect(classifyContextMode('code.exe', 'editor', 'general')).toBe('general');
    });
  });

  describe('getContextDirective', () => {
    it('generates developer directives with window title context', () => {
      const directive = getContextDirective('developer', 'VoiceTyping.tsx - sona');
      expect(directive).toContain('Developer & Engineering Mode');
      expect(directive).toContain('camelCase');
      expect(directive).toContain('VoiceTyping.tsx - sona');
    });

    it('generates chat directives without full stops', () => {
      const directive = getContextDirective('chat', 'Team Chat');
      expect(directive).toContain('Instant Messaging & Chat Mode');
      expect(directive).toContain('Never add trailing periods');
    });

    it('generates formal directives with structured punctuation', () => {
      const directive = getContextDirective('formal');
      expect(directive).toContain('Formal Writing & Document Mode');
      expect(directive).toContain('grammatically precise tone');
    });

    it('handles general mode cleanly', () => {
      expect(getContextDirective('general')).toBe('');
      expect(getContextDirective('general', 'My Document')).toContain('My Document');
    });
  });
});
