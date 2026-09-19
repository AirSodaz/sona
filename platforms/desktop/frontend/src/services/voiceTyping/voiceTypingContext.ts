import type {
  HostPlatform,
  VoiceTypingContextPreset,
  VoiceTypingContextRule,
} from '../../types/config';

export interface VoiceTypingContextState {
  appName: string;
  windowTitle: string;
  preset: VoiceTypingContextPreset;
  mode: string;
  rule?: VoiceTypingContextRule | null;
}

export const DEFAULT_VOICE_TYPING_CONTEXT_RULES: VoiceTypingContextRule[] = [
  {
    id: 'developer',
    name: 'Developer',
    icon: '💻',
    badgeColor: '#818cf8',
    appsByPlatform: {
      windows: [
        'code.exe',
        'cursor.exe',
        'idea64.exe',
        'devenv.exe',
        'clion64.exe',
        'pycharm64.exe',
        'webstorm64.exe',
        'goland64.exe',
        'rider64.exe',
        'sublime_text.exe',
        'zed.exe',
        'neovide.exe',
        'nvim.exe',
        'vim.exe',
        'emacs.exe',
        'windowsterminal.exe',
        'powershell.exe',
        'cmd.exe',
        'alacritty.exe',
        'wezterm-gui.exe',
        'kitty.exe',
        'warp.exe',
        'git-bash.exe',
        'conhost.exe',
        'mintty.exe',
        'postman.exe',
        'dbeaver.exe',
        'datagrip64.exe',
        'navicat.exe',
      ],
      macos: [
        'Visual Studio Code',
        'Code',
        'Xcode',
        'Cursor',
        'Zed',
        'Sublime Text',
        'IntelliJ IDEA',
        'CLion',
        'PyCharm',
        'WebStorm',
        'GoLand',
        'Rider',
        'DataGrip',
        'Postman',
        'DBeaver',
        'Terminal',
        'iTerm2',
        'Alacritty',
        'kitty',
        'Warp',
        'Neovide',
        'MacVim',
      ],
      linux: [
        'code',
        'cursor',
        'zed',
        'sublime_text',
        'idea',
        'clion',
        'pycharm',
        'webstorm',
        'goland',
        'rider',
        'datagrip',
        'postman',
        'dbeaver',
        'gnome-terminal',
        'konsole',
        'alacritty',
        'kitty',
        'wezterm',
        'warp-terminal',
        'xfce4-terminal',
      ],
    },
    titlePatterns: ['github', 'gitlab', 'stackoverflow', 'localhost', 'pull request', 'leetcode'],
    promptDirective:
      'Developer & Engineering Mode: Preserve technical terms, exact library/module names, and code identifiers (camelCase, PascalCase, snake_case, kebab-case, UPPER_CASE, CLI flags like --flag). Never spell out code symbols as prose. Keep it succinct and technical. Never add trailing periods or punctuation unless dictating explanatory comments.',
    stripTrailingPunctuation: true,
    isBuiltin: true,
    enabled: true,
  },
  {
    id: 'chat',
    name: 'Chat',
    icon: '💬',
    badgeColor: '#34d399',
    appsByPlatform: {
      windows: [
        'wechat.exe',
        'qq.exe',
        'slack.exe',
        'discord.exe',
        'telegram.exe',
        'dingtalk.exe',
        'feishu.exe',
        'teams.exe',
        'lark.exe',
        'whatsapp.exe',
        'signal.exe',
        'skype.exe',
        'line.exe',
        'element.exe',
      ],
      macos: [
        'WeChat',
        'QQ',
        'Slack',
        'Discord',
        'Telegram',
        'DingTalk',
        'Feishu',
        'Lark',
        'Microsoft Teams',
        'WhatsApp',
        'Signal',
        'Skype',
        'LINE',
        'Element',
        'Messages',
      ],
      linux: [
        'wechat',
        'qq',
        'slack',
        'discord',
        'telegram-desktop',
        'dingtalk',
        'feishu',
        'lark',
        'teams',
        'whatsapp-for-linux',
        'signal-desktop',
        'skypeforlinux',
        'element-desktop',
      ],
    },
    titlePatterns: ['slack |', 'discord', 'telegram', 'wechat', 'messages'],
    promptDirective:
      'Instant Messaging & Chat Mode: Use a natural, conversational, lightweight tone. Split long rambling speech into clear, compact phrases. Separate clauses with spaces rather than heavy commas. Never add trailing periods or full stops.',
    stripTrailingPunctuation: true,
    isBuiltin: true,
    enabled: true,
  },
  {
    id: 'formal',
    name: 'Formal',
    icon: '📄',
    badgeColor: '#f59e0b',
    appsByPlatform: {
      windows: [
        'winword.exe',
        'excel.exe',
        'powerpnt.exe',
        'outlook.exe',
        'foxmail.exe',
        'thunderbird.exe',
        'wps.exe',
        'wpp.exe',
        'et.exe',
        'notion.exe',
        'obsidian.exe',
        'typora.exe',
        'logseq.exe',
        'craft.exe',
        'acrobat.exe',
      ],
      macos: [
        'Microsoft Word',
        'Microsoft Excel',
        'Microsoft PowerPoint',
        'Pages',
        'Numbers',
        'Keynote',
        'Microsoft Outlook',
        'Mail',
        'Foxmail',
        'Thunderbird',
        'WPS Office',
        'Notion',
        'Obsidian',
        'Typora',
        'Logseq',
        'Craft',
        'TextEdit',
      ],
      linux: [
        'libreoffice',
        'soffice.bin',
        'wps',
        'wpp',
        'et',
        'notion-app',
        'obsidian',
        'typora',
        'thunderbird',
      ],
    },
    titlePatterns: ['document', 'report', 'notion', 'obsidian', 'word', 'excel', 'sheets', 'docs'],
    promptDirective:
      'Formal Writing & Document Mode: Convert colloquial spoken expressions into rigorous, well-structured, professional written language. Ensure complete grammatical sentence structures and proper punctuation.',
    stripTrailingPunctuation: false,
    isBuiltin: true,
    enabled: true,
  },
];

/** Detect the current host operating system. */
export function getCurrentPlatform(): HostPlatform {
  if (typeof navigator === 'undefined') {
    return 'windows';
  }
  const platform = (navigator.platform || '').toLowerCase();
  const ua = (navigator.userAgent || '').toLowerCase();

  if (platform.includes('mac') || ua.includes('macintosh') || ua.includes('mac os')) {
    return 'macos';
  }
  if (platform.includes('linux') || ua.includes('linux') || ua.includes('x11')) {
    return 'linux';
  }
  return 'windows';
}

/** Check if an application name or window title matches a context rule for a specific platform. */
export function matchContextRule(
  rule: VoiceTypingContextRule,
  appName: string,
  windowTitle: string,
  platform: HostPlatform
): boolean {
  if (!rule.enabled) {
    return false;
  }

  const cleanApp = appName.trim().toLowerCase();
  if (cleanApp) {
    const appList = rule.appsByPlatform[platform] || [];
    const matched = appList.some((target) => {
      const cleanTarget = target.trim().toLowerCase();
      if (cleanTarget === cleanApp) {
        return true;
      }
      if (platform === 'windows') {
        return cleanTarget.replace(/\.exe$/, '') === cleanApp.replace(/\.exe$/, '');
      }
      return false;
    });
    if (matched) {
      return true;
    }
  }

  const cleanTitle = windowTitle.trim().toLowerCase();
  if (cleanTitle) {
    const matchedTitle = rule.titlePatterns.some((pattern) => {
      const p = pattern.trim().toLowerCase();
      return p.length > 0 && cleanTitle.includes(p);
    });
    if (matchedTitle) {
      return true;
    }
  }

  return false;
}

/**
 * Classify and resolve the matching context rule.
 * If manual preset is not 'auto', the rule with that id is returned.
 * If no rule matches, returns null (general standard mode).
 */
export function classifyContextRule(
  appName: string,
  windowTitle: string,
  rules: VoiceTypingContextRule[] = DEFAULT_VOICE_TYPING_CONTEXT_RULES,
  platform: HostPlatform = getCurrentPlatform(),
  preset: VoiceTypingContextPreset = 'auto'
): VoiceTypingContextRule | null {
  if (preset && preset !== 'auto') {
    if (preset === 'general') {
      return null;
    }
    return rules.find((r) => r.id === preset) ?? null;
  }

  for (const rule of rules) {
    if (matchContextRule(rule, appName, windowTitle, platform)) {
      return rule;
    }
  }

  return null;
}

/**
 * Backwards-compatible helper returning rule id (or 'general').
 */
export function classifyContextMode(
  appName: string,
  windowTitle: string,
  preset: VoiceTypingContextPreset = 'auto',
  rules: VoiceTypingContextRule[] = DEFAULT_VOICE_TYPING_CONTEXT_RULES,
  platform: HostPlatform = getCurrentPlatform()
): string {
  const rule = classifyContextRule(appName, windowTitle, rules, platform, preset);
  return rule ? rule.id : 'general';
}

/**
 * Generate contextual prompt directive from a matched rule or mode.
 */
export function getContextDirective(
  ruleOrMode: VoiceTypingContextRule | string | null | undefined,
  windowTitle?: string,
  rules: VoiceTypingContextRule[] = DEFAULT_VOICE_TYPING_CONTEXT_RULES
): string {
  let rule: VoiceTypingContextRule | null | undefined = null;
  if (ruleOrMode && typeof ruleOrMode === 'object') {
    rule = ruleOrMode;
  } else if (typeof ruleOrMode === 'string' && ruleOrMode !== 'general' && ruleOrMode !== 'auto') {
    rule = rules.find((r) => r.id === ruleOrMode);
  }

  const trimmedTitle = windowTitle?.trim();
  const titleContext = trimmedTitle
    ? `Active window context and topic: "${trimmedTitle}". Disambiguate technical terms and domain abbreviations accordingly.`
    : '';

  if (!rule) {
    return titleContext;
  }

  return titleContext ? `${titleContext}\n${rule.promptDirective}` : rule.promptDirective;
}

/**
 * Check if trailing punctuation should be stripped based on rule or mode.
 */
export function shouldStripTrailingPunctuation(
  ruleOrMode: VoiceTypingContextRule | string | null | undefined,
  rules: VoiceTypingContextRule[] = DEFAULT_VOICE_TYPING_CONTEXT_RULES
): boolean {
  if (!ruleOrMode) {
    return false;
  }
  if (typeof ruleOrMode === 'object') {
    return Boolean(ruleOrMode.stripTrailingPunctuation);
  }
  const rule = rules.find((r) => r.id === ruleOrMode);
  return Boolean(rule?.stripTrailingPunctuation);
}
