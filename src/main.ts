import {
  addServer,
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
import type { FailedDeletion, Server } from './backend';
import {
  failureMessage,
  openFailureMessage,
  removeFailureMessage,
  renameSavedMessage,
  storeDeletionMessage,
  subjectMessage,
  testResultMessage,
  urlMessage,
} from './messages';
import { clearFor, flaggedServers } from './notices';
import type { Notice, NoticeAction, NoticeCause } from './notices';
import { renderPanel } from './panel';
import { focusRename, markFlagged, renderServerList, revealRow } from './server-list';
import { checkServerUrl } from './server-url';
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
const form = element('add-server', HTMLFormElement);
const nameInput = element('server-name', HTMLInputElement);
const urlInput = element('server-url', HTMLInputElement);

let notices: readonly Notice[] = [];
let noticeCounter = 0;
let servers: readonly Server[] = [];
const failedDeletions = new Map<string, FailedDeletion>();
const retrying = new Set<string>();

function announce(text: string): void {
  live.textContent = '';
  requestAnimationFrame(() => {
    live.textContent = text;
  });
}

function renderHeader(): void {
  count.textContent = String(servers.length);
  renderPanel(panel, notices, [...failedDeletions.values()], retrying, {
    dismiss: (notice) => {
      notices = notices.filter((candidate) => candidate.id !== notice.id);
      renderHeader();
      markFlagged(list, flaggedServers(notices));
    },
    retry: retryDeletion,
  });
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
  if (tone === 'error' && serverId !== null) revealRow(list, serverId);
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

async function finishRename(server: Server): Promise<void> {
  await refresh();
  focusRename(list, server.id);
}

async function removeAndReload(server: Server): Promise<void> {
  clear('remove');
  const removal = await removeServer(server.id).then(
    () => null,
    (failure: unknown) => ({ failure }),
  );
  await refresh().catch((failure: unknown) => {
    reportStatus('remove', failure);
  });
  if (removal !== null) {
    notify('remove', null, 'error', removeFailureMessage(server.name, removal.failure));
  }
}

async function refresh(): Promise<void> {
  servers = await listServers();
  renderServerList(list, emptyState, servers, flaggedServers(notices), {
    test: async (server) => {
      clear('test');
      try {
        await testServer(server.id);
        notify('test', server.id, 'ok', testResultMessage(server.name, null));
      } catch (failure) {
        notify('test', server.id, 'error', testResultMessage(server.name, failure));
      }
    },
    open: async (server) => {
      clear('open');
      try {
        await openServer(server.id);
      } catch (failure) {
        notify('open', server.id, 'error', openFailureMessage(server.name, failure));
      }
    },
    remove: (server) => {
      removeAndReload(server).catch((failure: unknown) => {
        reportStatus('remove', failure);
      });
    },
    rename: async (server, name) => {
      clear('rename');
      try {
        await renameServer(server.id, name);
        await finishRename(server);
        return true;
      } catch (failure) {
        const saved = renameSavedMessage(failure);
        if (saved === null) {
          notify(
            'rename',
            server.id,
            'error',
            subjectMessage(server.name, failureMessage(failure)),
          );
          return false;
        }
        await finishRename(server).catch((reload: unknown) => {
          reportStatus('rename', reload);
        });
        notify('rename', server.id, 'error', subjectMessage(name.trim(), saved));
        return true;
      }
    },
    cancelRename: () => {
      clear('cancelRename');
    },
  });
  renderHeader();
}

async function submit(): Promise<void> {
  clear('add');
  const checked = checkServerUrl(urlInput.value);
  if (!checked.ok) {
    notify('add', null, 'error', urlMessage(checked.error));
    urlInput.focus();
    return;
  }

  await addServer(nameInput.value, checked.url);
  form.reset();
  await refresh();
  nameInput.focus();
}

form.addEventListener('submit', (event) => {
  event.preventDefault();
  submit().catch((failure: unknown) => {
    reportStatus('add', failure);
  });
});

function reportStartup(failure: unknown): void {
  reportStatus('startup', failure);
}

Promise.all([
  onServersChanged(() => {
    refresh().catch(reportStartup);
  }),
  onOpenFailed(() => {
    showOpenFailure().catch(reportStartup);
  }),
])
  .then(() => Promise.all([refresh(), showOpenFailure()]))
  .catch(reportStartup);

trackStoreDeletions().catch(reportStartup);
