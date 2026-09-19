import { describe, expect, it } from 'vitest';
import {
  addTermToYamlDictionary,
  extractHotwordWeight,
  formatHotwordWithWeight,
  formatYamlDictionary,
  getTermsForProject,
  isVoiceTypingScope,
  isVoiceTypingScopeId,
  isVoiceTypingScopeName,
  parseYamlDictionary,
  removeTermFromYamlDictionary,
  serializeToYamlDictionary,
  updateTermInYamlDictionary,
  VOICE_TYPING_SCOPE_ID,
  validateYamlDictionary,
} from '../yamlDictionaryParser';

describe('yamlDictionaryParser', () => {
  const mockProjects = [
    { id: 'proj-1', name: 'Weekly Review' },
    { id: 'proj-2', name: 'Medical Research' },
  ];

  it('parses global terms, weighted hotwords, and arrow replacements without colon conflict', () => {
    const yaml = `
# Global Terms
- Sona
- DeepSeek
- Sherpa-onnx :2.0
- 苦伯内提斯 -> Kubernetes
- 微信支付 => WeChat Pay
`;
    const parsed = parseYamlDictionary(yaml, mockProjects);

    expect(parsed.globalTerms).toHaveLength(5);
    expect(parsed.globalTerms[0].text).toBe('Sona');
    expect(parsed.globalTerms[0].isReplacement).toBe(false);

    expect(parsed.globalTerms[2].text).toBe('Sherpa-onnx :2.0');
    expect(parsed.globalTerms[2].isReplacement).toBe(false);

    expect(parsed.globalTerms[3].from).toBe('苦伯内提斯');
    expect(parsed.globalTerms[3].to).toBe('Kubernetes');
    expect(parsed.globalTerms[3].isReplacement).toBe(true);

    expect(parsed.globalTerms[4].from).toBe('微信支付');
    expect(parsed.globalTerms[4].to).toBe('WeChat Pay');
    expect(parsed.globalTerms[4].isReplacement).toBe(true);
  });

  it('parses project specific sections with explicit project id annotations', () => {
    const yaml = `
- GlobalTerm

projects:
  Weekly Review (id:proj-1):
    - Sprint Planning
    - PRD -> Product Requirements Document

  Medical Research (id:proj-2):
    - Stent
`;
    const parsed = parseYamlDictionary(yaml, mockProjects);

    expect(parsed.globalTerms).toHaveLength(1);
    expect(parsed.projects).toHaveLength(2);

    expect(parsed.projects[0].projectId).toBe('proj-1');
    expect(parsed.projects[0].projectName).toBe('Weekly Review');
    expect(parsed.projects[0].terms).toHaveLength(2);
    expect(parsed.projects[0].terms[0].text).toBe('Sprint Planning');
    expect(parsed.projects[0].terms[1].from).toBe('PRD');
    expect(parsed.projects[0].terms[1].to).toBe('Product Requirements Document');

    expect(parsed.projects[1].projectId).toBe('proj-2');
    expect(parsed.projects[1].terms).toHaveLength(1);
  });

  it('synchronizes project display name upon formatting when project is renamed', () => {
    const yaml = `
projects:
  Old Project Title (id:proj-1):
    - Sprint
`;
    // Project with ID proj-1 has been renamed to 'Weekly Review' in mockProjects
    const formatted = formatYamlDictionary(yaml, mockProjects);

    expect(formatted).toContain('Weekly Review (id:proj-1):');
    expect(formatted).not.toContain('Old Project Title');
  });

  it('warns when a linked project was deleted and does not confuse with same-name new project', () => {
    const yaml = `
projects:
  Weekly Review (id:old-deleted-id):
    - Sprint
`;
    // In mockProjects, 'Weekly Review' now has ID 'proj-1'
    const diagnostics = validateYamlDictionary(yaml, mockProjects);

    expect(diagnostics).toHaveLength(1);
    expect(diagnostics[0].severity).toBe('warning');
    expect(diagnostics[0].message).toContain('was deleted');
    expect(diagnostics[0].message).toContain('id:proj-1');
  });

  it('detects syntax errors such as multiple arrows or missing colons', () => {
    const invalidYaml = `
- Word -> Another -> Third
projects:
  MissingColon
    - Term
`;
    const diagnostics = validateYamlDictionary(invalidYaml, mockProjects);

    expect(diagnostics.some((d) => d.message.includes('Multiple replacement arrows'))).toBe(true);
    expect(diagnostics.some((d) => d.message.includes('must end with a colon'))).toBe(true);
  });

  it('extracts combined terms and replacements for a specific project', () => {
    const yaml = `
- GlobalHotword
- GlobalMistake -> GlobalFix

projects:
  Weekly Review (id:proj-1):
    - ProjectHotword
    - ProjectMistake -> ProjectFix
`;
    const parsed = parseYamlDictionary(yaml, mockProjects);
    const result = getTermsForProject(parsed, 'proj-1');

    expect(result.hotwords).toContain('GlobalHotword');
    expect(result.hotwords).toContain('GlobalFix');
    expect(result.hotwords).toContain('ProjectHotword');
    expect(result.hotwords).toContain('ProjectFix');

    expect(result.replacements).toEqual([
      { from: 'GlobalMistake', to: 'GlobalFix' },
      { from: 'ProjectMistake', to: 'ProjectFix' },
    ]);
  });

  it('serializes legacy rule sets and project pipeline terms into YAML', () => {
    const hotwordSets = [
      {
        id: 'hw-1',
        name: 'Tech',
        enabled: true,
        rules: [{ id: '1', text: 'Sona' }],
      },
    ];
    const textReplacementSets = [
      {
        id: 'rep-1',
        name: 'Fixes',
        enabled: true,
        ignoreCase: false,
        rules: [{ id: '1', from: 'K8s', to: 'Kubernetes' }],
      },
    ];
    const projectsWithTerms = [
      {
        id: 'proj-1',
        name: 'Weekly Review',
        pipeline: {
          customTerms: ['Roadmap', 'Bug => Issue'],
        },
      },
    ];

    const yaml = serializeToYamlDictionary(hotwordSets, textReplacementSets, projectsWithTerms);

    expect(yaml).toContain('- Sona');
    expect(yaml).toContain('- K8s -> Kubernetes');
    expect(yaml).toContain('Weekly Review (id:proj-1):');
    expect(yaml).toContain('- Roadmap');
    expect(yaml).toContain('- Bug -> Issue');
  });

  it('identifies voice typing scope by id and localized names', () => {
    expect(isVoiceTypingScopeId('voice-typing')).toBe(true);
    expect(isVoiceTypingScopeId('voice_typing')).toBe(true);
    expect(isVoiceTypingScopeId('other-project')).toBe(false);

    expect(isVoiceTypingScopeName('Voice Typing')).toBe(true);
    expect(isVoiceTypingScopeName('语音输入')).toBe(true);
    expect(isVoiceTypingScopeName('語音輸入')).toBe(true);
    expect(isVoiceTypingScopeName('音声入力')).toBe(true);
    expect(isVoiceTypingScopeName('음성 입력')).toBe(true);
    expect(isVoiceTypingScopeName('Other Project')).toBe(false);

    expect(isVoiceTypingScope('voice-typing', 'Any')).toBe(true);
    expect(isVoiceTypingScope(undefined, '语音输入')).toBe(true);
  });

  it('parses and validates voice typing scope as a special project-exclusive scope without errors', () => {
    const yaml = `
projects:
  Voice Typing (id:voice-typing):
    - Whisper
    - GPT -> ChatGPT

  语音输入:
    - 豆包 -> 字节豆包
`;
    const parsed = parseYamlDictionary(yaml, mockProjects);

    expect(parsed.projects).toHaveLength(2);
    expect(parsed.projects[0].projectId).toBe(VOICE_TYPING_SCOPE_ID);
    expect(parsed.projects[0].isDeleted).toBe(false);
    expect(parsed.projects[0].terms).toHaveLength(2);

    expect(parsed.projects[1].projectId).toBe(VOICE_TYPING_SCOPE_ID);
    expect(parsed.projects[1].isDeleted).toBe(false);
    expect(parsed.projects[1].terms).toHaveLength(1);

    // Validation should not treat voice-typing as deleted or missing project
    const diagnostics = validateYamlDictionary(yaml, mockProjects);
    const errorOrWarn = diagnostics.filter(
      (d) => d.severity === 'error' || d.severity === 'warning'
    );
    expect(errorOrWarn).toHaveLength(0);
  });

  it('formats voice typing scope cleanly with id:voice-typing', () => {
    const yaml = `
projects:
  语音输入:
    - Hotword
`;
    const formatted = formatYamlDictionary(yaml, mockProjects);
    expect(formatted).toContain('  语音输入 (id:voice-typing):');
    expect(formatted).toContain('    - Hotword');
  });

  it('extracts terms for voice typing scope including global terms', () => {
    const yaml = `
- GlobalHotword
- GlobalMistake -> GlobalFix

projects:
  Voice Typing (id:voice-typing):
    - DictationHotword
    - VT_Mistake -> VT_Fix

  Weekly Review (id:proj-1):
    - ProjectOnlyHotword
`;
    const parsed = parseYamlDictionary(yaml, mockProjects);
    const terms = getTermsForProject(parsed, VOICE_TYPING_SCOPE_ID);

    expect(terms.hotwords).toContain('GlobalHotword');
    expect(terms.hotwords).toContain('GlobalFix');
    expect(terms.hotwords).toContain('DictationHotword');
    expect(terms.hotwords).toContain('VT_Fix');
    expect(terms.hotwords).not.toContain('ProjectOnlyHotword');

    expect(terms.replacements).toEqual([
      { from: 'GlobalMistake', to: 'GlobalFix' },
      { from: 'VT_Mistake', to: 'VT_Fix' },
    ]);
  });

  it('adds terms to global and voice typing scope using addTermToYamlDictionary', () => {
    // 1. Add to empty dictionary under voice typing scope
    const emptyDict = '';
    const withVT = addTermToYamlDictionary(
      emptyDict,
      'VoiceHotword',
      { id: VOICE_TYPING_SCOPE_ID, name: 'Voice Typing' },
      mockProjects
    );
    expect(withVT).toContain('projects:');
    expect(withVT).toContain('Voice Typing (id:voice-typing):');
    expect(withVT).toContain('- VoiceHotword');

    // 2. Add replacement rule to existing voice typing scope
    const withReplacement = addTermToYamlDictionary(
      withVT,
      'Misrecognized => Corrected',
      { id: VOICE_TYPING_SCOPE_ID, name: 'Voice Typing' },
      mockProjects
    );
    expect(withReplacement).toContain('- Misrecognized -> Corrected');

    // 3. Deduplication: adding the same term should not duplicate
    const duplicateAdd = addTermToYamlDictionary(
      withReplacement,
      'VoiceHotword',
      { id: VOICE_TYPING_SCOPE_ID, name: 'Voice Typing' },
      mockProjects
    );
    expect(duplicateAdd).toBe(withReplacement);

    // 4. Add to global scope
    const withGlobal = addTermToYamlDictionary(
      withReplacement,
      'GlobalTerm',
      'global',
      mockProjects
    );
    expect(withGlobal).toContain('- GlobalTerm');
    // Global term should be before projects:
    const lines = withGlobal.split('\n');
    const globalIdx = lines.findIndex((l) => l.trim() === '- GlobalTerm');
    const projectsIdx = lines.findIndex((l) => l.trim() === 'projects:');
    expect(globalIdx).toBeLessThan(projectsIdx);
  });

  it('removes terms from global and project scopes using removeTermFromYamlDictionary', () => {
    const dict = `# Global
- Sona
- DeepSeek
- 苦伯内提斯 -> Kubernetes

projects:
  Voice Typing (id:voice-typing):
    - VoiceWord
    - OldVoice -> NewVoice
`;
    // Remove global hotword
    const afterRemoveGlobalHotword = removeTermFromYamlDictionary(
      dict,
      { text: 'DeepSeek', isReplacement: false },
      'global',
      mockProjects
    );
    expect(afterRemoveGlobalHotword).not.toContain('- DeepSeek');
    expect(afterRemoveGlobalHotword).toContain('- Sona');

    // Remove global replacement
    const afterRemoveGlobalRep = removeTermFromYamlDictionary(
      afterRemoveGlobalHotword,
      { from: '苦伯内提斯', to: 'Kubernetes', isReplacement: true },
      'global',
      mockProjects
    );
    expect(afterRemoveGlobalRep).not.toContain('苦伯内提斯 -> Kubernetes');

    // Remove voice typing term
    const afterRemoveVTTerm = removeTermFromYamlDictionary(
      afterRemoveGlobalRep,
      { text: 'VoiceWord', isReplacement: false },
      VOICE_TYPING_SCOPE_ID,
      mockProjects
    );
    expect(afterRemoveVTTerm).not.toContain('- VoiceWord');
    expect(afterRemoveVTTerm).toContain('- OldVoice -> NewVoice');
  });

  it('updates terms in global and project scopes using updateTermInYamlDictionary', () => {
    const dict = `- Sona
projects:
  Voice Typing (id:voice-typing):
    - VoiceWord
`;
    const updatedGlobal = updateTermInYamlDictionary(
      dict,
      { text: 'Sona', isReplacement: false },
      'Sona AI',
      'global',
      mockProjects
    );
    expect(updatedGlobal).toContain('- Sona AI');
    expect(updatedGlobal).not.toContain('- Sona\n');

    const updatedVT = updateTermInYamlDictionary(
      updatedGlobal,
      { text: 'VoiceWord', isReplacement: false },
      'VoiceWord => CorrectedWord',
      VOICE_TYPING_SCOPE_ID,
      mockProjects
    );
    expect(updatedVT).toContain('    - VoiceWord -> CorrectedWord');
  });

  it('extracts and formats hotword weights correctly', () => {
    expect(extractHotwordWeight('sherpa-onnx :2.0')).toEqual({
      word: 'sherpa-onnx',
      weight: 2.0,
    });
    expect(extractHotwordWeight('Sona')).toEqual({
      word: 'Sona',
    });
    expect(formatHotwordWithWeight('sherpa-onnx', 2.0)).toBe('sherpa-onnx :2.0');
    expect(formatHotwordWithWeight('Sona', '')).toBe('Sona');
  });
});
