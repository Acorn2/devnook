import { useEffect, useState } from 'react';

type ProjectPresentation = {
  version: 1;
  view: 'grid' | 'compact' | 'table';
  groupBy: 'none' | 'category';
  sort: 'name' | 'recent';
  collapsedCategories: string[];
};

const storageKey = 'devnook.projects.presentation';
const defaults: ProjectPresentation = {
  version: 1,
  view: 'compact',
  groupBy: 'none',
  sort: 'recent',
  collapsedCategories: [],
};

function readPresentation(): ProjectPresentation {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(storageKey) || 'null');
    if (!saved || typeof saved !== 'object' || !('version' in saved) || saved.version !== 1)
      return defaults;
    const value = saved as Partial<ProjectPresentation>;
    return {
      ...defaults,
      view: value.view === 'grid' || value.view === 'table' ? value.view : 'compact',
      groupBy: value.groupBy === 'category' ? 'category' : 'none',
      sort: value.sort === 'name' ? 'name' : 'recent',
      collapsedCategories: Array.isArray(value.collapsedCategories)
        ? [
            ...new Set(
              value.collapsedCategories.filter((item): item is string => typeof item === 'string'),
            ),
          ]
        : [],
    };
  } catch {
    return defaults;
  }
}

export function useProjectPresentation() {
  const [presentation, setPresentation] = useState(readPresentation);
  const [storageError, setStorageError] = useState(false);
  useEffect(() => {
    try {
      localStorage.setItem(storageKey, JSON.stringify(presentation));
      setStorageError(false);
    } catch {
      setStorageError(true);
    }
  }, [presentation]);
  return { presentation, setPresentation, storageError };
}
