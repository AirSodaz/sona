import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function setAuxWindowState<T>(label: string, payload: T): Promise<void> {
  await invokeTauri(TauriCommand.system.setAuxWindowState, { label, payload });
}

export async function getAuxWindowState<T>(label: string): Promise<T | null> {
  return invokeTauri(TauriCommand.system.getAuxWindowState, { label }) as Promise<T | null>;
}

export async function clearAuxWindowState(label: string): Promise<void> {
  await invokeTauri(TauriCommand.system.clearAuxWindowState, { label });
}

export async function injectText(
  text: string,
  shortcutModifiers?: string[],
): Promise<void> {
  await invokeTauri(TauriCommand.system.injectText, shortcutModifiers?.length
    ? { text, shortcutModifiers }
    : { text });
}

export async function getMousePosition(): Promise<[number, number]> {
  return invokeTauri(TauriCommand.system.getMousePosition);
}

export async function getTextCursorPosition(): Promise<[number, number] | null> {
  return invokeTauri(TauriCommand.system.getTextCursorPosition);
}
