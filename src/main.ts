import {
  failedStoreDeletions,
  listServers,
  onOpenFailed,
  onServersChanged,
  onStoreDeletionFailed,
  onStoreDeletionSucceeded,
  openServer,
  removeServer,
  renameServer,
  retryStoreDeletion,
  takeOpenFailure,
  testServer,
} from './backend';
import { createAddSheet } from './add-sheet';
import type { FailedDeletion, Server } from './backend';
import { focusedKey, restoreFocus, revealNearest } from './dom';
import {
  failureMessage,
  openFailureMessage,
  removeFailureMessage,
  renameSavedMessage,
  storeDeletionMessage,
  subjectMessage,
  testResultMessage,
} from './messages';
import { clearFor, flaggedServers } from './notices';
import type { Notice, NoticeAction, NoticeCause } from './notices';
import { renderPanel } from './panel';
import { findRow, markFlagged, renderServerList } from './server-list';
import type { Editing, ListHandlers, ListView } from './server-list';
import './styles.css';

function element<T extends HTMLElement>(id: string, type: new () => T): T {
  const found = document.getElementById(id);
  if (!(found instanceof type)) throw new Error(`index.html has no #${id}`);
  return found;
}

const panel = element('panel', HTMLUListElement);
const count = element('count', HTMLElement);
const live = element('live', HTMLParagraphElement);
const list = element('servers', HTMLUListElement);
const emptyState = element('empty', HTMLParagraphElement);
const addTrigger = element('add-trigger', HTMLButtonElement);
const addDialog = element('add-sheet', HTMLDialogElement);

let notices: readonly Notice[] = [];
let noticeCounter = 0;
const failedDeletions = new Map<string, FailedDeletion>();
const retrying = new Set<string>();

const pending = new Map<string, 'test' | 'open'>();
const view: { -readonly [K in keyof ListView]: ListView[K] } = {
  servers: [],
  editing: null,
  confirming: null,
  menuOpen: null,
  pending,
  removing: null,
  flagged: new Set(),
};
let stale = false;

function announce(text: string): void {
  live.textContent = '';
  requestAnimationFrame(() => {
    live.textContent = text;
  });
}

function renderHeader(): void {
  count.textContent = String(view.servers.length);
  renderPanel(panel, notices, [...failedDeletions.values()], retrying, {
    dismiss: (notice) => {
      notices = notices.filter((candidate) => candidate.id !== notice.id);
      renderHeader();
      markFlagged(list, flaggedServers(notices));
    },
    retry: retryDeletion,
  });
}

interface ListRender {
  readonly focus?: string;
  readonly select?: boolean;
  readonly reuseEditor?: boolean;
}

function renderList(options: ListRender = {}): void {
  const before = focusedKey(list);
  view.flagged = flaggedServers(notices);
  renderServerList(list, emptyState, view, handlers, { reuseEditor: options.reuseEditor ?? true });
  restoreFocus(list, options.focus ?? before, { select: options.select });
}

let pointerDown = false;
let renderWhenPointerUp = false;

function renderListSoon(): void {
  if (pointerDown) {
    renderWhenPointerUp = true;
    return;
  }
  setTimeout(() => {
    renderList();
  }, 0);
}

function clear(action: NoticeAction): void {
  notices = clearFor(notices, action);
  renderHeader();
  markFlagged(list, flaggedServers(notices));
}

function notify(
  cause: NoticeCause,
  serverId: string | null,
  tone: Notice['tone'],
  text: string,
): void {
  noticeCounter += 1;
  notices = [...notices, { id: noticeCounter, cause, serverId, tone, text }];
  renderHeader();
  panel.scrollTop = 0;
  markFlagged(list, flaggedServers(notices));
  if (tone === 'error' && serverId !== null) revealNearest(findRow(list, serverId));
  announce(text);
}

function reportStatus(cause: NoticeCause, failure: unknown): void {
  notify(cause, null, 'error', failureMessage(failure));
}

function retryDeletion(failure: FailedDeletion): void {
  clear('retry');
  retrying.add(failure.id);
  renderHeader();
  retryStoreDeletion(failure.id).catch((error: unknown) => {
    retrying.delete(failure.id);
    renderHeader();
    notify(
      'retry',
      null,
      'error',
      subjectMessage(failure.name ?? 'A removed server', failureMessage(error)),
    );
  });
}

function addDeletion(failure: FailedDeletion): void {
  retrying.delete(failure.id);
  failedDeletions.set(failure.id, failure);
  renderHeader();
  panel.scrollTop = 0;
  announce(storeDeletionMessage(failure.name));
}

function trackStoreDeletions(): Promise<void> {
  return Promise.all([
    onStoreDeletionFailed(addDeletion),
    onStoreDeletionSucceeded((id) => {
      retrying.delete(id);
      failedDeletions.delete(id);
      renderHeader();
    }),
  ]).then(async () => {
    for (const failure of await failedStoreDeletions()) {
      addDeletion(failure);
    }
  });
}

async function showOpenFailure(): Promise<void> {
  const failure = await takeOpenFailure();
  if (failure === null) return;
  notify('openedLink', null, 'error', openFailureMessage(failure.name, failure.error));
}

function present(id: string): boolean {
  return view.servers.some((server) => server.id === id);
}

async function loadList(options: ListRender = {}): Promise<void> {
  view.servers = await listServers();
  if (view.confirming !== null && !present(view.confirming)) view.confirming = null;
  if (view.menuOpen !== null && !present(view.menuOpen)) view.menuOpen = null;
  renderList(options);
  renderHeader();
}

function requestReload(): Promise<void> {
  if (view.editing !== null) {
    stale = true;
    return Promise.resolve();
  }
  return loadList();
}

function endEdit(): void {
  if (view.editing === null) return;
  view.editing = null;
  clear('cancelRename');
  if (stale) {
    stale = false;
    loadList().catch((failure: unknown) => {
      reportStatus('startup', failure);
    });
  }
}

function lostFocus(): boolean {
  return document.activeElement === null || document.activeElement === document.body;
}

async function runTest(server: Server): Promise<void> {
  clear('test');
  view.menuOpen = null;
  pending.set(server.id, 'test');
  renderList({ focus: `menu-${server.id}` });
  try {
    await testServer(server.id);
    notify('test', server.id, 'ok', testResultMessage(server.name, null));
  } catch (failure) {
    notify('test', server.id, 'error', testResultMessage(server.name, failure));
  }
  pending.delete(server.id);
  renderList({ focus: lostFocus() && view.editing === null ? `menu-${server.id}` : undefined });
}

async function runOpen(server: Server): Promise<void> {
  clear('open');
  pending.set(server.id, 'open');
  renderList();
  try {
    await openServer(server.id);
  } catch (failure) {
    notify('open', server.id, 'error', openFailureMessage(server.name, failure));
  }
  pending.delete(server.id);
  renderList({ focus: lostFocus() && view.editing === null ? `open-${server.id}` : undefined });
}

function startRename(server: Server): void {
  endEdit();
  view.menuOpen = null;
  view.confirming = null;
  view.editing = { id: server.id, value: server.name, saving: false, invalid: false };
  renderList({ focus: 'rename-input', select: true });
}

function cancelRename(): void {
  const editing = view.editing;
  if (editing === null || editing.saving) return;
  endEdit();
  renderList({ focus: `menu-${editing.id}` });
}

async function saveRename(): Promise<void> {
  const editing: Editing | null = view.editing;
  const server = view.servers.find((candidate) => candidate.id === editing?.id);
  if (editing === null || editing.saving || server === undefined) return;

  clear('rename');
  editing.saving = true;
  editing.invalid = false;
  renderList({ reuseEditor: false });

  let savedWithWarning: string | null = null;
  try {
    await renameServer(server.id, editing.value);
  } catch (failure) {
    savedWithWarning = renameSavedMessage(failure);
    if (savedWithWarning === null) {
      editing.saving = false;
      editing.invalid = true;
      notify('rename', server.id, 'error', subjectMessage(server.name, failureMessage(failure)));
      renderList(view.editing === editing ? { focus: 'rename-input', reuseEditor: false } : {});
      return;
    }
  }

  if (view.editing === editing) {
    view.editing = null;
    stale = false;
  }
  await loadList().catch((failure: unknown) => {
    reportStatus('rename', failure);
    renderList();
  });
  if (view.editing === null) restoreFocus(list, `menu-${server.id}`);
  if (savedWithWarning !== null) {
    notify('rename', server.id, 'error', subjectMessage(editing.value.trim(), savedWithWarning));
  }
}

async function removeAndReload(server: Server): Promise<void> {
  clear('remove');
  const index = view.servers.findIndex((candidate) => candidate.id === server.id);
  view.removing = server.id;
  renderList();
  const removal = await removeServer(server.id).then(
    () => null,
    (failure: unknown) => ({ failure }),
  );
  view.removing = null;
  view.confirming = null;
  await loadList().catch((failure: unknown) => {
    reportStatus('remove', failure);
    renderList();
  });
  if (removal !== null) {
    notify('remove', null, 'error', removeFailureMessage(server.name, removal.failure));
  }
  const next = view.servers[index] ?? view.servers[index - 1];
  if (next === undefined) addTrigger.focus();
  else restoreFocus(list, `open-${next.id}`);
}

const handlers: ListHandlers = {
  open: (server) => {
    runOpen(server).catch((failure: unknown) => {
      reportStatus('open', failure);
    });
  },
  openMenu: (server, itemIndex) => {
    endEdit();
    view.menuOpen = server.id;
    renderList({ focus: `item-${server.id}-${String(itemIndex)}` });
  },
  closeMenu: (server) => {
    view.menuOpen = null;
    renderList({ focus: `menu-${server.id}` });
  },
  test: (server) => {
    runTest(server).catch((failure: unknown) => {
      reportStatus('test', failure);
    });
  },
  startRename,
  editValue: (value) => {
    if (view.editing !== null) view.editing.value = value;
  },
  saveRename: () => {
    saveRename().catch((failure: unknown) => {
      reportStatus('rename', failure);
    });
  },
  cancelRename,
  askRemove: (server) => {
    view.menuOpen = null;
    view.confirming = server.id;
    renderList({ focus: `confirm-cancel-${server.id}` });
  },
  cancelRemove: (server) => {
    if (view.removing !== null) return;
    view.confirming = null;
    renderList({ focus: `menu-${server.id}` });
  },
  remove: (server) => {
    removeAndReload(server).catch((failure: unknown) => {
      reportStatus('remove', failure);
    });
  },
  disarmRemove: () => {
    if (view.confirming === null || view.removing !== null) return;
    view.confirming = null;
    renderListSoon();
  },
};

document.addEventListener('pointerdown', (event) => {
  pointerDown = true;
  const target = event.target;
  if (!(target instanceof Element)) return;
  let changed = false;
  if (
    view.menuOpen !== null &&
    target.closest(`[data-menu="${CSS.escape(view.menuOpen)}"]`) === null
  ) {
    view.menuOpen = null;
    changed = true;
  }
  if (
    view.confirming !== null &&
    view.removing === null &&
    target.closest('[data-mode="confirming"]') === null
  ) {
    view.confirming = null;
    changed = true;
  }
  if (changed) renderListSoon();
});

function releasePointer(): void {
  pointerDown = false;
  if (!renderWhenPointerUp) return;
  renderWhenPointerUp = false;
  renderListSoon();
}
document.addEventListener('pointerup', releasePointer);
document.addEventListener('pointercancel', releasePointer);

const addSheet = createAddSheet(addDialog, {
  trigger: addTrigger,
  onAdded: async (added) => {
    try {
      await loadList();
      revealNearest(findRow(list, added.id));
    } catch (failure) {
      reportStatus('startup', failure);
    }
  },
});

addTrigger.addEventListener('click', () => {
  if (view.editing !== null) {
    endEdit();
    renderList();
  }
  addSheet.open();
});

function reportStartup(failure: unknown): void {
  reportStatus('startup', failure);
}

Promise.all([
  onServersChanged(() => {
    requestReload().catch(reportStartup);
  }),
  onOpenFailed(() => {
    showOpenFailure().catch(reportStartup);
  }),
])
  .then(() => Promise.all([loadList(), showOpenFailure()]))
  .catch(reportStartup);

trackStoreDeletions().catch(reportStartup);
