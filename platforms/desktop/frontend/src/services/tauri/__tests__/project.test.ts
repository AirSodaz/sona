import { describe, expect, it, vi } from 'vitest';
import * as tauriInvoke from '../invoke';
import { projectList } from '../project';

describe('tauri project normalization', () => {
  it('maps autoPolishFrequency from the wire record', async () => {
    vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue([
      {
        id: 'p1',
        name: 'P',
        description: '',
        icon: '',
        color: undefined,
        sortOrder: 0,
        createdAt: 1,
        updatedAt: 2,
        pipeline: { enabled: true, autoPolish: true, autoPolishFrequency: 10 },
      },
    ] as never);

    const [record] = await projectList();
    expect(record.pipeline?.autoPolishFrequency).toBe(10);
  });
});
