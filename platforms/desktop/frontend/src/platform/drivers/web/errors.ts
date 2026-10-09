export class UnsupportedCapabilityError extends Error {
  readonly command: string;
  readonly environment: string;
  readonly code: string = 'UNSUPPORTED_CAPABILITY';

  constructor(command: string, reason?: string) {
    super(
      reason ||
        `[UnsupportedCapabilityError] The command "${command}" is a desktop-only capability and is not supported in the web browser environment.`
    );
    this.name = 'UnsupportedCapabilityError';
    this.command = command;
    this.environment = 'web';
  }
}
