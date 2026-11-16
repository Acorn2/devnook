// Presentation only: a project's color survives sorting, filtering and language changes.
// It never represents process identity, ownership, readiness or control permission.
const projectColors = ['blue', 'violet', 'teal', 'amber', 'rose', 'orange'] as const;
export type LabelColor = (typeof projectColors)[number] | 'neutral';

export function projectColor(id?: string | null): LabelColor {
  if (!id) return 'neutral';
  let hash = 2166136261;
  for (let i = 0; i < id.length; i++) hash = Math.imul(hash ^ id.charCodeAt(i), 16777619);
  return projectColors[(hash >>> 0) % projectColors.length];
}

const technologyColors: Record<string, LabelColor> = {
  NODE: 'teal',
  'NODE.JS': 'teal',
  JAVA: 'orange',
  PYTHON: 'blue',
  RUST: 'orange',
  GO: 'teal',
  DOTNET: 'violet',
  PHP: 'violet',
  RUBY: 'rose',
  DART: 'blue',
  SWIFT: 'orange',
  CPP: 'blue',
  MIXED: 'violet',
  REACT: 'blue',
  'NEXT.JS': 'violet',
  VUE: 'teal',
  NUXT: 'teal',
  NESTJS: 'rose',
  'SPRING BOOT': 'teal',
  DJANGO: 'teal',
  FASTAPI: 'teal',
  FLASK: 'blue',
  SVELTE: 'orange',
  ANGULAR: 'rose',
  VITE: 'violet',
  VITEPRESS: 'violet',
  AXUM: 'orange',
  TAURI: 'amber',
  ELECTRON: 'blue',
};

export const technologyColor = (name: string): LabelColor =>
  technologyColors[name.toUpperCase()] || 'neutral';
