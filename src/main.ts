import {
  addServer,
  failedStoreDeletions,
  listServers,
  onStoreDeletionFailed,
  onStoreDeletionSucceeded,
  openServer,
  removeServer,
  renameServer,
  retryStoreDeletion,
} from './backend';
import type { FailedDeletion } from './backend';
import { failureMessage, urlMessage } from './messages';
import { focusRename, renderDeletionErrors, renderServerList } from './server-list';
import { checkServerUrl } from './server-url';
import './styles.css';

function element<T extends HTMLElement>(id: string, type: new () => T): T {
  const found = document.getElementById(id);
  if (!(found instanceof type)) throw new Error(`index.html has no #${id}`);
  return found;
}

const deletionErrors = element('deletion-errors', HTMLUListElement);
const list = element('servers', HTMLUListElement);
const emptyState = element('empty', HTMLParagraphElement);
const form = element('add-server', HTMLFormElement);
const nameInput = element('server-name', HTMLInputElement);
const urlInput = element('server-url', HTMLInputElement);
const status = element('status', HTMLParagraphElement);

function report(message: string): void {
  status.textContent = message;
}

const failedDeletions = new Map<string, FailedDeletion>();
const retrying = new Set<string>();

function renderFailedDeletions(): void {
  renderDeletionErrors(deletionErrors, [...failedDeletions.values()], retrying, (failure) => {
    report('');
    retrying.add(failure.id);
    renderFailedDeletions();
    retryStoreDeletion(failure.id).catch((error: unknown) => {
      retrying.delete(failure.id);
      renderFailedDeletions();
      report(failureMessage(error));
    });
  });
}

function trackStoreDeletions(): Promise<void> {
  return Promise.all([
    onStoreDeletionFailed((failure) => {
      retrying.delete(failure.id);
      failedDeletions.set(failure.id, failure);
      renderFailedDeletions();
    }),
    onStoreDeletionSucceeded((id) => {
      retrying.delete(id);
      failedDeletions.delete(id);
      renderFailedDeletions();
    }),
  ]).then(async () => {
    for (const failure of await failedStoreDeletions()) {
      failedDeletions.set(failure.id, failure);
    }
    renderFailedDeletions();
  });
}

async function refresh(): Promise<void> {
  const servers = await listServers();
  renderServerList(list, emptyState, servers, {
    open: (server) => {
      report('');
      openServer(server.id).catch((failure: unknown) => {
        report(failureMessage(failure));
      });
    },
    remove: (server) => {
      report('');
      removeServer(server.id)
        .then(refresh)
        .catch((failure: unknown) => {
          report(failureMessage(failure));
        });
    },
    rename: async (server, name) => {
      report('');
      try {
        await renameServer(server.id, name);
        await refresh();
        focusRename(list, server.id);
        return true;
      } catch (failure) {
        report(failureMessage(failure));
        return false;
      }
    },
  });
}

async function submit(): Promise<void> {
  const checked = checkServerUrl(urlInput.value);
  if (!checked.ok) {
    report(urlMessage(checked.error));
    urlInput.focus();
    return;
  }

  await addServer(nameInput.value, checked.url);
  form.reset();
  report('');
  await refresh();
  nameInput.focus();
}

form.addEventListener('submit', (event) => {
  event.preventDefault();
  submit().catch((failure: unknown) => {
    report(failureMessage(failure));
  });
});

refresh().catch((failure: unknown) => {
  report(failureMessage(failure));
});

trackStoreDeletions().catch((failure: unknown) => {
  report(failureMessage(failure));
});
