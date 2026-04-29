import React from 'react';

type IconModule = Record<string, unknown>;

export function createNamedIconMock(name: string) {
  return function MockIcon(): React.JSX.Element {
    return <span>{name}</span>;
  };
}

export async function buildPartialIconsMock<TModule extends IconModule>(
  importOriginal: () => Promise<TModule>,
  overrides: Partial<TModule>,
): Promise<TModule> {
  return {
    ...(await importOriginal()),
    ...overrides,
  };
}
