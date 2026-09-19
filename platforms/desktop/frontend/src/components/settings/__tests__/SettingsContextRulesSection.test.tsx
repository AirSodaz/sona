import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfigStore } from '../../../stores/configStore';
import { SettingsContextRulesSection } from '../SettingsContextRulesSection';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue ?? key,
  }),
}));
vi.mock('../../../services/voiceTyping/voiceTypingContext', async (importOriginal) => {
  const mod =
    await importOriginal<typeof import('../../../services/voiceTyping/voiceTypingContext')>();
  return {
    ...mod,
    getCurrentPlatform: () => 'windows',
  };
});

describe('SettingsContextRulesSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        voiceTypingContextAwarenessEnabled: true,
        voiceTypingContextPreset: 'auto',
      },
    });
  });

  it('renders default context rules list with card items', () => {
    render(<SettingsContextRulesSection />);

    expect(screen.getByTestId('voice-typing-rules-list')).toBeTruthy();
    expect(screen.getByTestId('voice-typing-rule-card-developer')).toBeTruthy();
    expect(screen.getByTestId('voice-typing-rule-card-chat')).toBeTruthy();
    expect(screen.getByTestId('voice-typing-rule-card-formal')).toBeTruthy();
    expect(screen.getByText('Developer')).toBeTruthy();
    expect(screen.getByText('Chat')).toBeTruthy();
    expect(screen.getByText('Formal')).toBeTruthy();
  });

  it('toggles rule enabled state when clicking switch', () => {
    render(<SettingsContextRulesSection />);

    const devCard = screen.getByTestId('voice-typing-rule-card-developer');
    const switchBtn = devCard.querySelector('[role="switch"]') as HTMLElement;
    expect(switchBtn).toBeTruthy();

    fireEvent.click(switchBtn);
    const updatedRules = useConfigStore.getState().config.voiceTypingContextRules;
    const devRule = updatedRules?.find((r) => r.id === 'developer');
    expect(devRule?.enabled).toBe(false);
  });

  it('opens edit modal and displays apps for current platform', async () => {
    render(<SettingsContextRulesSection />);

    const devCard = screen.getByTestId('voice-typing-rule-card-developer');
    const editBtn = devCard.querySelectorAll('button')[0]; // edit button
    fireEvent.click(editBtn);

    expect(screen.getByText('Edit Situation: Developer')).toBeTruthy();
    expect(screen.getByTestId('app-chip-code.exe')).toBeTruthy();
  });

  it('adds and removes application chips within the edit modal', async () => {
    render(<SettingsContextRulesSection />);

    const devCard = screen.getByTestId('voice-typing-rule-card-developer');
    const editBtn = devCard.querySelectorAll('button')[0];
    fireEvent.click(editBtn);

    const appInput = screen.getByPlaceholderText('e.g. code.exe, slack.exe');
    fireEvent.change(appInput, { target: { value: 'custom_editor.exe' } });
    const addBtn = screen.getByTestId('add-app-btn');
    fireEvent.click(addBtn);

    expect(screen.getByTestId('app-chip-custom_editor.exe')).toBeTruthy();

    // Remove the added chip
    const chip = screen.getByTestId('app-chip-custom_editor.exe');
    const removeBtn = chip.querySelector('button')!;
    fireEvent.click(removeBtn);

    expect(screen.queryByTestId('app-chip-custom_editor.exe')).toBeNull();
  });

  it('selects preset emoji icon when clicked in edit modal', async () => {
    render(<SettingsContextRulesSection />);

    const devCard = screen.getByTestId('voice-typing-rule-card-developer');
    const editBtn = devCard.querySelectorAll('button')[0];
    fireEvent.click(editBtn);

    const emojiBtn = screen.getByTitle('⚡');
    fireEvent.click(emojiBtn);

    const iconInput = screen.getByDisplayValue('⚡');
    expect(iconInput).toBeTruthy();
  });

  it('adds a new custom situation and saves it into config', async () => {
    render(<SettingsContextRulesSection />);

    const addSituationBtn = screen.getByTestId('add-context-rule-btn');
    fireEvent.click(addSituationBtn);

    expect(screen.getByText('Add Custom Situation')).toBeTruthy();

    const nameInput = screen.getByPlaceholderText('e.g. Email / Customer Service');
    fireEvent.change(nameInput, { target: { value: 'Email Reply' } });

    const saveBtn = screen.getByRole('button', { name: 'Save' });
    fireEvent.click(saveBtn);

    const rules = useConfigStore.getState().config.voiceTypingContextRules;
    const addedRule = rules?.find((r) => r.name === 'Email Reply');
    expect(addedRule).toBeDefined();
    expect(addedRule?.isBuiltin).toBe(false);
  });
});
