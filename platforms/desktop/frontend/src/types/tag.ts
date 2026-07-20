import type { TagRecord as GeneratedTagRecord } from '../bindings';

export type TagRecord = Omit<GeneratedTagRecord, 'color' | 'sortOrder'> & {
  color?: string;
  sortOrder?: number;
};

export type TagCreateInput = {
  name: string;
  description?: string;
  icon?: string;
  color?: string;
};

export type TagUpdateInput = {
  name?: string;
  icon?: string;
  color?: string;
  description?: string;
};
