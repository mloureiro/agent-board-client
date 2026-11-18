import {
  addServer,
  listServers,
  openServer,
  removeServer,
  setServerNotifications,
} from './backend';
import { failureMessage, urlMessage } from './messages';
import { renderServerList } from './server-list';
import { checkServerUrl } from './server-url';
import './styles.css';

function element<T extends HTMLElement>(id: string, type: new () => T): T {
  const found = document.getElementById(id);
  if (!(found instanceof type)) throw new Error(`index.html has no #${id}`);
  return found;
}

const list = element('servers', HTMLUListElement);
const emptyState = element('empty', HTMLParagraphElement);
const form = element('add-server', HTMLFormElement);
const nameInput = element('server-name', HTMLInputElement);
const urlInput = element('server-url', HTMLInputElement);
const status = element('status', HTMLParagraphElement);

function report(message: string): void {
  status.textContent = message;
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
    setNotifications: (server, enabled) => {
      report('');
      setServerNotifications(server.id, enabled)
        .catch((failure: unknown) => {
          report(failureMessage(failure));
          return refresh();
        })
        .catch((failure: unknown) => {
          report(failureMessage(failure));
        });
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
