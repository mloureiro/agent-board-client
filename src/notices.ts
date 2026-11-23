export type NoticeCause =
  'test' | 'open' | 'openedLink' | 'remove' | 'rename' | 'retry' | 'add' | 'startup';

export interface Notice {
  readonly id: number;
  readonly cause: NoticeCause;
  readonly serverId: string | null;
  readonly tone: 'ok' | 'error';
  readonly text: string;
}

export type NoticeAction = 'test' | 'open' | 'remove' | 'rename' | 'cancelRename' | 'retry' | 'add';

const STATUS_CAUSES: ReadonlySet<NoticeCause> = new Set([
  'remove',
  'rename',
  'retry',
  'add',
  'startup',
]);

export function clearFor(notices: readonly Notice[], action: NoticeAction): Notice[] {
  if (action === 'test' || action === 'open') return [];
  return notices.filter((notice) => !STATUS_CAUSES.has(notice.cause));
}

export function flaggedServers(notices: readonly Notice[]): ReadonlySet<string> {
  const flagged = new Set<string>();
  for (const notice of notices) {
    if (notice.tone === 'error' && notice.serverId !== null) flagged.add(notice.serverId);
  }
  return flagged;
}
