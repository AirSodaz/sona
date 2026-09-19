export type VoiceTypingContextPreset = 'auto' | 'general' | 'developer' | 'chat' | 'formal';

export type ResolvedVoiceTypingContextMode = 'general' | 'developer' | 'chat' | 'formal';

export interface VoiceTypingContextState {
  appName: string;
  windowTitle: string;
  preset: VoiceTypingContextPreset;
  mode: ResolvedVoiceTypingContextMode;
}

const DEVELOPER_APP_NAMES: Record<string, true> = {
  // Windows executables
  'code.exe': true,
  'cursor.exe': true,
  'idea64.exe': true,
  'devenv.exe': true,
  'clion64.exe': true,
  'pycharm64.exe': true,
  'webstorm64.exe': true,
  'goland64.exe': true,
  'rider64.exe': true,
  'sublime_text.exe': true,
  'zed.exe': true,
  'neovide.exe': true,
  'nvim.exe': true,
  'vim.exe': true,
  'emacs.exe': true,
  'windowsterminal.exe': true,
  'powershell.exe': true,
  'cmd.exe': true,
  'alacritty.exe': true,
  'wezterm-gui.exe': true,
  'kitty.exe': true,
  'warp.exe': true,
  'git-bash.exe': true,
  'conhost.exe': true,
  'mintty.exe': true,
  'postman.exe': true,
  'dbeaver.exe': true,
  'datagrip64.exe': true,
  'navicat.exe': true,
  // macOS / Linux base names & binaries
  code: true,
  'visual studio code': true,
  xcode: true,
  cursor: true,
  zed: true,
  sublime_text: true,
  'sublime text': true,
  idea: true,
  clion: true,
  pycharm: true,
  webstorm: true,
  goland: true,
  rider: true,
  datagrip: true,
  dbeaver: true,
  postman: true,
  navicat: true,
  neovide: true,
  nvim: true,
  vim: true,
  emacs: true,
  terminal: true,
  iterm: true,
  iterm2: true,
  alacritty: true,
  wezterm: true,
  kitty: true,
  warp: true,
  'gnome-terminal': true,
  konsole: true,
  xfce4_terminal: true,
};

const CHAT_APP_NAMES: Record<string, true> = {
  // Windows executables
  'wechat.exe': true,
  'qq.exe': true,
  'slack.exe': true,
  'discord.exe': true,
  'telegram.exe': true,
  'dingtalk.exe': true,
  'feishu.exe': true,
  'teams.exe': true,
  'lark.exe': true,
  'whatsapp.exe': true,
  'signal.exe': true,
  'skype.exe': true,
  'line.exe': true,
  'element.exe': true,
  // macOS / Linux base names
  wechat: true,
  qq: true,
  slack: true,
  discord: true,
  telegram: true,
  'telegram desktop': true,
  dingtalk: true,
  feishu: true,
  lark: true,
  teams: true,
  'microsoft teams': true,
  whatsapp: true,
  signal: true,
  skype: true,
  line: true,
  element: true,
  messages: true,
};

const FORMAL_APP_NAMES: Record<string, true> = {
  // Windows executables
  'winword.exe': true,
  'excel.exe': true,
  'powerpnt.exe': true,
  'outlook.exe': true,
  'foxmail.exe': true,
  'thunderbird.exe': true,
  'wps.exe': true,
  'wpp.exe': true,
  'et.exe': true,
  'notion.exe': true,
  'obsidian.exe': true,
  'typora.exe': true,
  'logseq.exe': true,
  'craft.exe': true,
  'acrobat.exe': true,
  // macOS / Linux base names
  winword: true,
  word: true,
  'microsoft word': true,
  excel: true,
  'microsoft excel': true,
  powerpnt: true,
  powerpoint: true,
  'microsoft powerpoint': true,
  pages: true,
  numbers: true,
  keynote: true,
  outlook: true,
  'microsoft outlook': true,
  mail: true,
  foxmail: true,
  thunderbird: true,
  wps: true,
  notion: true,
  obsidian: true,
  typora: true,
  logseq: true,
  craft: true,
  textedit: true,
  libreoffice: true,
  soffice: true,
};

function normalizeAppKey(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/\.exe$/i, '')
    .replace(/(?:64|32)$/i, '');
}

export function classifyContextMode(
  appName: string,
  windowTitle: string,
  preset: VoiceTypingContextPreset = 'auto'
): ResolvedVoiceTypingContextMode {
  if (preset !== 'auto') {
    return preset;
  }

  const normalizedApp = appName.trim().toLowerCase();
  const baseApp = normalizeAppKey(normalizedApp);
  const normalizedTitle = windowTitle.trim().toLowerCase();

  if (DEVELOPER_APP_NAMES[normalizedApp] || DEVELOPER_APP_NAMES[baseApp]) {
    return 'developer';
  }

  if (CHAT_APP_NAMES[normalizedApp] || CHAT_APP_NAMES[baseApp]) {
    return 'chat';
  }

  if (FORMAL_APP_NAMES[normalizedApp] || FORMAL_APP_NAMES[baseApp]) {
    return 'formal';
  }
  // Fallback heuristic based on title if app is a generic browser or wrapper
  if (
    normalizedTitle.includes('github') ||
    normalizedTitle.includes('gitlab') ||
    normalizedTitle.includes('stackoverflow') ||
    normalizedTitle.includes('localhost') ||
    normalizedTitle.includes('visual studio code')
  ) {
    return 'developer';
  }

  if (
    normalizedTitle.includes('slack |') ||
    normalizedTitle.includes('discord') ||
    normalizedTitle.includes('telegram') ||
    normalizedTitle.includes('wechat')
  ) {
    return 'chat';
  }

  return 'general';
}

export function getContextDirective(
  mode: ResolvedVoiceTypingContextMode,
  windowTitle?: string
): string {
  const trimmedTitle = windowTitle?.trim();
  const titleContext = trimmedTitle
    ? `Active window context and topic: "${trimmedTitle}". Disambiguate technical terms and domain abbreviations accordingly.`
    : '';

  switch (mode) {
    case 'developer':
      return [
        '[Developer & Engineering Mode]',
        '1. Preserve code identifiers and casing verbatim (camelCase, snake_case, PascalCase, SCREAMING_SNAKE).',
        '2. Maintain technical terms, CLI flags, URLs, and keyboard shortcuts exactly as intended.',
        '3. Do not add trailing full stops or unnecessary punctuation to code, commands, or identifiers.',
        titleContext,
      ]
        .filter(Boolean)
        .join('\n');

    case 'chat':
      return [
        '[Instant Messaging & Chat Mode]',
        '1. Use a natural, conversational, and concise tone with shorter phrases.',
        '2. Never add trailing periods or full stops at the end of sentences.',
        '3. Use spaces rather than rigid commas to separate brief conversational thoughts.',
        titleContext,
      ]
        .filter(Boolean)
        .join('\n');

    case 'formal':
      return [
        '[Formal Writing & Document Mode]',
        '1. Maintain an authoritative, polished, and grammatically precise tone.',
        '2. Ensure standard, structured punctuation throughout.',
        '3. Eliminate filler words and colloquialisms, transforming them into clear written prose.',
        titleContext,
      ]
        .filter(Boolean)
        .join('\n');

    case 'general':
    default:
      return titleContext;
  }
}
