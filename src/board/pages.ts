import nativePages from '../../fixtures/native-pages.json';

export const PAGE_KINDS = ['home', 'backlog', 'closed', 'schedules', 'task', 'settings'] as const;
export type PageKind = (typeof PAGE_KINDS)[number];

const STATIC_PAGES: Readonly<Record<string, PageKind>> = {
  '/': 'home',
  '/backlog': 'backlog',
  '/schedules': 'schedules',
  '/closed': 'closed',
};

function isPageKind(value: unknown): value is PageKind {
  return PAGE_KINDS.some((kind) => kind === value);
}

export function pageKindOf(path: string): PageKind | null {
  const pathname = path.split(/[?#]/)[0] ?? '';
  const trimmed = pathname.length > 1 && pathname.endsWith('/') ? pathname.slice(0, -1) : pathname;

  const known = STATIC_PAGES[trimmed];
  if (known) {
    return known;
  }
  if (trimmed === '/settings' || trimmed.startsWith('/settings/')) {
    return 'settings';
  }
  if (trimmed.startsWith('/t/')) {
    const id = trimmed.slice('/t/'.length);
    return id !== '' && !id.includes('/') ? 'task' : null;
  }
  return null;
}

export function parseNativePages(value: unknown): readonly PageKind[] {
  if (!Array.isArray(value)) {
    throw new TypeError('The native page table must be an array.');
  }
  return value.map((entry: unknown) => {
    if (!isPageKind(entry)) {
      throw new TypeError(`Unknown page kind in the native page table: ${String(entry)}`);
    }
    return entry;
  });
}

export function isNativePath(path: string, pages: readonly PageKind[]): boolean {
  const kind = pageKindOf(path);
  return kind !== null && pages.includes(kind);
}

export const NATIVE_PAGES: readonly PageKind[] = parseNativePages(nativePages);
