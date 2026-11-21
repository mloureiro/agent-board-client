import type { FailedDeletion, Server } from './backend';
import { storeDeletionMessage } from './messages';

export interface ServerListActions {
  readonly test: (server: Server) => Promise<void>;
  readonly open: (server: Server) => Promise<void>;
  readonly remove: (server: Server) => void;
  readonly rename: (server: Server, name: string) => Promise<boolean>;
}

let cancelActiveEdit: (() => void) | null = null;

function button(label: string, className: string, onClick: () => void): HTMLButtonElement {
  const element = document.createElement('button');
  element.type = 'button';
  element.className = className;
  element.textContent = label;
  element.addEventListener('click', onClick);
  return element;
}

function pendingButton(
  label: string,
  pendingLabel: string,
  className: string,
  run: () => Promise<void>,
): HTMLButtonElement {
  const element = button(label, className, () => {
    element.disabled = true;
    element.textContent = pendingLabel;
    void run().finally(() => {
      element.disabled = false;
      element.textContent = label;
    });
  });
  return element;
}

function removeButton(server: Server, remove: (server: Server) => void): HTMLButtonElement {
  const element = button('Remove', 'quiet', () => {
    if (element.dataset.armed === 'true') {
      remove(server);
      return;
    }
    element.dataset.armed = 'true';
    element.textContent = 'Confirm remove';
  });
  element.addEventListener('blur', () => {
    delete element.dataset.armed;
    element.textContent = 'Remove';
  });
  return element;
}

function renameForm(
  server: Server,
  rename: ServerListActions['rename'],
  stop: (restoreFocus: boolean) => void,
): HTMLFormElement {
  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'rename-input';
  input.value = server.name;
  input.autocomplete = 'off';
  input.setAttribute('aria-label', `Name for ${server.url}`);
  input.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    stop(true);
  });

  const save = document.createElement('button');
  save.type = 'submit';
  save.className = 'primary';
  save.textContent = 'Save';

  const form = document.createElement('form');
  form.className = 'rename';
  form.noValidate = true;
  form.append(
    input,
    save,
    button('Cancel', 'quiet', () => {
      stop(true);
    }),
  );
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    if (save.disabled) return;
    save.disabled = true;
    rename(server, input.value)
      .then((saved) => {
        if (saved) return;
        save.disabled = false;
        input.focus();
      })
      .catch(() => {
        save.disabled = false;
      });
  });
  return form;
}

function serverRow(server: Server, actions: ServerListActions): HTMLLIElement {
  const name = document.createElement('span');
  name.className = 'server-name';
  name.textContent = server.name;

  const url = document.createElement('span');
  url.className = 'server-url';
  url.textContent = server.url;
  url.title = server.url;

  const identity = document.createElement('div');
  identity.className = 'server-identity';
  identity.append(name, url);

  const renameButton = button('Rename', 'quiet', () => {
    cancelActiveEdit?.();
    const stop = (restoreFocus: boolean): void => {
      identity.replaceChildren(name, url);
      renameButton.hidden = false;
      cancelActiveEdit = null;
      if (restoreFocus) renameButton.focus();
    };
    const form = renameForm(server, actions.rename, stop);
    identity.replaceChildren(form, url);
    renameButton.hidden = true;
    cancelActiveEdit = () => {
      stop(false);
    };
    const input = form.querySelector('input');
    input?.focus();
    input?.select();
  });
  renameButton.dataset.rename = 'true';

  const controls = document.createElement('div');
  controls.className = 'server-controls';
  controls.append(
    renameButton,
    removeButton(server, actions.remove),
    pendingButton('Test', 'Testing…', '', () => actions.test(server)),
    pendingButton('Open', 'Connecting…', 'primary', () => actions.open(server)),
  );

  const row = document.createElement('li');
  row.className = 'server';
  row.dataset.serverId = server.id;
  row.append(identity, controls);
  return row;
}

export function renderServerList(
  list: HTMLElement,
  emptyState: HTMLElement,
  servers: readonly Server[],
  actions: ServerListActions,
): void {
  cancelActiveEdit = null;
  list.replaceChildren(...servers.map((server) => serverRow(server, actions)));
  emptyState.hidden = servers.length > 0;
}

function deletionRow(
  failure: FailedDeletion,
  retrying: boolean,
  retry: (failure: FailedDeletion) => void,
): HTMLLIElement {
  const message = document.createElement('span');
  message.textContent = storeDeletionMessage(failure.name);

  const retryButton = button('Retry', 'primary', () => {
    retry(failure);
  });
  retryButton.disabled = retrying;

  const row = document.createElement('li');
  row.className = 'deletion-error';
  row.dataset.serverId = failure.id;
  row.append(message, retryButton);
  return row;
}

export function renderDeletionErrors(
  container: HTMLElement,
  failures: readonly FailedDeletion[],
  retrying: ReadonlySet<string>,
  retry: (failure: FailedDeletion) => void,
): void {
  container.replaceChildren(
    ...failures.map((failure) => deletionRow(failure, retrying.has(failure.id), retry)),
  );
}

export function focusRename(list: HTMLElement, serverId: string): void {
  for (const row of list.children) {
    if (row instanceof HTMLElement && row.dataset.serverId === serverId) {
      row.querySelector<HTMLButtonElement>('button[data-rename]')?.focus();
    }
  }
}
