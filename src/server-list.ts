import type { Server } from './backend';

export interface ServerListActions {
  readonly open: (server: Server) => void;
  readonly remove: (server: Server) => void;
  readonly setNotifications: (server: Server, enabled: boolean) => void;
}

function button(label: string, className: string, onClick: () => void): HTMLButtonElement {
  const element = document.createElement('button');
  element.type = 'button';
  element.className = className;
  element.textContent = label;
  element.addEventListener('click', onClick);
  return element;
}

function notificationSwitch(
  server: Server,
  setNotifications: (server: Server, enabled: boolean) => void,
): HTMLLabelElement {
  const input = document.createElement('input');
  input.type = 'checkbox';
  input.checked = server.notifications.enabled;
  input.addEventListener('change', () => {
    setNotifications(server, input.checked);
  });

  const caption = document.createElement('span');
  caption.textContent = 'Notifications';

  const element = document.createElement('label');
  element.className = 'server-notifications';
  element.append(input, caption);
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
  identity.append(name, url, notificationSwitch(server, actions.setNotifications));

  const controls = document.createElement('div');
  controls.className = 'server-controls';
  controls.append(
    button('Remove', 'quiet', () => {
      actions.remove(server);
    }),
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
