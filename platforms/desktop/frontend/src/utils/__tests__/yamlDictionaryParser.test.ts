import { describe, expect, it } from 'vitest';
import {
  formatYamlDictionary,
  getTermsForProject,
  parseYamlDictionary,
  serializeToYamlDictionary,
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
});
