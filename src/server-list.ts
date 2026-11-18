import type { Server } from './backend';

export interface ServerListActions {
  readonly open: (server: Server) => void;
  readonly remove: (server: Server) => void;
}

function button(label: string, className: string, onClick: () => void): HTMLButtonElement {
  const element = document.createElement('button');
  element.type = 'button';
  element.className = className;
  element.textContent = label;
  element.addEventListener('click', onClick);
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

  const controls = document.createElement('div');
  controls.className = 'server-controls';
  controls.append(
    removeButton(server, actions.remove),
    button('Open', 'primary', () => {
      actions.open(server);
    }),
  );

  const row = document.createElement('li');
  row.className = 'server';
  row.append(identity, controls);
  return row;
}

export function renderServerList(
  list: HTMLElement,
  emptyState: HTMLElement,
  servers: readonly Server[],
  actions: ServerListActions,
): void {
  list.replaceChildren(...servers.map((server) => serverRow(server, actions)));
  emptyState.hidden = servers.length > 0;
}
