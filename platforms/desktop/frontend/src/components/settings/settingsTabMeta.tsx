import { BarChart3, Bot, Cloud, HardDrive, Server } from 'lucide-react';
import type React from 'react';
import type { SettingsTab } from '../../types/settings';
import {
  AutomationIcon,
  BookIcon,
  GeneralIcon,
  InfoIcon,
  KeyboardIcon,
  MicIcon,
  ModelIcon,
  RobotIcon,
  SubtitleIcon,
} from '../Icons';

export interface SettingsTabMetaItem {
  labelKey: string;
  defaultValue?: string;
  Icon: () => React.JSX.Element;
}

export const SETTINGS_TAB_META: Record<SettingsTab, SettingsTabMetaItem> = {
  general: { labelKey: 'settings.general', defaultValue: 'General', Icon: GeneralIcon },
  dashboard: {
    labelKey: 'settings.dashboard.title',
    defaultValue: 'Dashboard',
    Icon: () => <BarChart3 size={18} />,
  },
  shortcuts: { labelKey: 'shortcuts.title', defaultValue: 'Shortcuts', Icon: KeyboardIcon },
  about: { labelKey: 'settings.about', defaultValue: 'About', Icon: InfoIcon },
  microphone: { labelKey: 'settings.input_device', defaultValue: 'Input Device', Icon: MicIcon },
  models: { labelKey: 'settings.model_hub', defaultValue: 'Model Settings', Icon: ModelIcon },
  subtitle: {
    labelKey: 'settings.subtitle_voice_typing_title',
    defaultValue: 'Subtitles & Voice Typing',
    Icon: SubtitleIcon,
  },
  vocabulary: { labelKey: 'settings.vocabulary', defaultValue: 'Vocabulary', Icon: BookIcon },
  llm_service: { labelKey: 'settings.llm.title', defaultValue: 'LLM Service', Icon: RobotIcon },
  automation: { labelKey: 'settings.automation', defaultValue: 'Automation', Icon: AutomationIcon },
  storage: {
    labelKey: 'settings.storage.title',
    defaultValue: 'Storage',
    Icon: () => <HardDrive size={18} />,
  },
  sync: {
    labelKey: 'settings.sync.title',
    defaultValue: 'Cloud Sync',
    Icon: () => <Cloud size={18} />,
  },
  api_server: {
    labelKey: 'settings.api_server.title',
    defaultValue: 'API Server',
    Icon: () => <Server size={18} />,
  },
  mcp: {
    labelKey: 'settings.mcp.title',
    defaultValue: 'MCP Agents',
    Icon: () => <Bot size={18} />,
  },
};
