import { t } from './i18n';
import { call } from './api';
import type { Project } from './types';

export async function setProjectCategories(
  ids: string[],
  category: string,
  progress: (completed: number) => void,
) {
  const completed = new Set<string>();
  let projects: Project[] | null = null;
  let error = '';
  const target = category.trim();
  if (new TextEncoder().encode(target).length > 80)
    return { projects, completed, error: t('分类名称过长，请缩短后重试。') };
  for (const id of new Set(ids)) {
    let name = id;
    try {
      // Use the existing Core protocol without replacing the running backend.
      projects = await call<Project[]>('list_projects');
      const project = projects.find((item) => item.id === id);
      if (!project) throw new Error(t('项目已移除，请刷新列表。'));
      name = project.name;
      if (project.category !== target) {
        projects = await call<Project[]>('edit_project', {
          edit: {
            id,
            name: project.name,
            default_ide: project.default_ide,
            is_favorite: project.is_favorite,
            category: target,
          },
        });
      }
      completed.add(id);
      progress(completed.size);
    } catch (e) {
      error = `${name}：${e instanceof Error ? e.message : String(e)}`;
      break;
    }
  }
  return { projects, completed, error };
}
