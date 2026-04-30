import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function setAuxWindowState<T>(label: string, payload: T): Promise<void> {
  await invokeTauri<void>(TauriCommand.system.setAuxWindowState, { label, payload });
}

export async function getAuxWindowState<T>(label: string): Promise<T | null> {
  return invokeTauri<T | null>(TauriCommand.system.getAuxWindowState, { label });
}

export async function clearAuxWindowState(label: string): Promise<void> {
  await invokeTauri<void>(TauriCommand.system.clearAuxWindowState, { label });
}

export async function injectText(
  text: string,
  shortcutModifiers?: string[],
): Promise<void> {
  await invokeTauri<void>(TauriCommand.system.injectText, shortcutModifiers?.length
    ? { text, shortcutModifiers }
    : { text });
}

export async function getMousePosition(): Promise<[number, number]> {
  return invokeTauri<[number, number]>(TauriCommand.system.getMousePosition);
}

export async function getTextCursorPosition(): Promise<[number, number] | null> {
  return invokeTauri<[number, number] | null>(TauriCommand.system.getTextCursorPosition);
}
