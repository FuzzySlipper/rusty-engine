export type StudioStartupProject =
  | { readonly status: 'none' }
  | { readonly status: 'open'; readonly root: string; readonly projectFile: string }
  | { readonly status: 'invalid'; readonly diagnostic: string };

// Startup query values are host path selectors. These ceilings reject
// malformed URLs before they become adapter requests; they are not transfer
// budgets for project content.
const MAX_ROOT_LENGTH = 4096;
const MAX_PROJECT_FILE_LENGTH = 1024;

export function readStudioStartupProject(href: string): StudioStartupProject {
  let url: URL;
  try {
    url = new URL(href, 'http://127.0.0.1/');
  } catch {
    return { status: 'invalid', diagnostic: 'Studio startup URL is malformed.' };
  }
  const roots = url.searchParams.getAll('root');
  const files = url.searchParams.getAll('project');
  if (roots.length === 0 && files.length === 0) return { status: 'none' };
  if (roots.length !== 1 || files.length !== 1) {
    return {
      status: 'invalid',
      diagnostic: 'Startup requires exactly one external root and one project-relative file.',
    };
  }
  const root = roots[0]?.trim() ?? '';
  const projectFile = files[0]?.trim() ?? '';
  if (
    root.length === 0
    || root.length > MAX_ROOT_LENGTH
    || projectFile.length === 0
    || projectFile.length > MAX_PROJECT_FILE_LENGTH
    || root.includes('\0')
    || projectFile.includes('\0')
  ) {
    return { status: 'invalid', diagnostic: 'Startup project selection is empty or malformed.' };
  }
  return { status: 'open', root, projectFile };
}
