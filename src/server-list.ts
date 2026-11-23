import type { Server } from './backend';
import { h } from './dom';
import { menu, menuButton } from './row-menu';

export interface Editing {
  readonly id: string;
  value: string;
  saving: boolean;
  invalid: boolean;
}

export interface ListView {
  readonly servers: readonly Server[];
  readonly editing: Editing | null;
  readonly confirming: string | null;
  readonly menuOpen: string | null;
  readonly pending: ReadonlyMap<string, 'test' | 'open'>;
  readonly removing: string | null;
  readonly flagged: ReadonlySet<string>;
}

export interface ListHandlers {
  readonly open: (server: Server) => void;
  readonly openMenu: (server: Server, itemIndex: number) => void;
  readonly closeMenu: (server: Server) => void;
  readonly test: (server: Server) => void;
  readonly startRename: (server: Server) => void;
  readonly editValue: (value: string) => void;
  readonly saveRename: () => void;
  readonly cancelRename: () => void;
  readonly askRemove: (server: Server) => void;
  readonly cancelRemove: (server: Server) => void;
  readonly remove: (server: Server) => void;
  readonly disarmRemove: () => void;
}

export interface RenderOptions {
  readonly reuseEditor?: boolean;
}

const MENU_ITEM_COUNT = 3;

function identity(server: Server): HTMLElement {
  return h(
    'div',
    { class: 'identity' },
    h('span', { class: 'name', text: server.name }),
    h('span', { class: 'url', text: server.url, title: server.url }),
  );
}

function normalRow(server: Server, view: ListView, handlers: ListHandlers): HTMLLIElement {
  const pending = view.pending.get(server.id);
  const menuOpen = view.menuOpen === server.id;
  return h(
    'li',
    {
      class: 'server',
      'data-server-id': server.id,
      'data-mode': 'normal',
      'data-flagged': view.flagged.has(server.id),
    },
    identity(server),
    pending === 'test' && h('span', { class: 'pending', text: 'Testing…' }),
    h('button', {
      type: 'button',
      class: 'primary',
      key: `open-${server.id}`,
      disabled: pending === 'open',
      text: pending === 'open' ? 'Connecting…' : 'Open',
      onclick: () => {
        handlers.open(server);
      },
    }),
    menuButton({
      id: server.id,
      label: `Actions for ${server.name}`,
      expanded: menuOpen,
      disabled: pending === 'test',
      onOpen: (at) => {
        handlers.openMenu(server, at === 'first' ? 0 : MENU_ITEM_COUNT - 1);
      },
      onClose: () => {
        handlers.closeMenu(server);
      },
    }),
    menuOpen &&
      menu({
        id: server.id,
        onClose: () => {
          handlers.closeMenu(server);
        },
        items: [
          {
            label: 'Test',
            run: () => {
              handlers.test(server);
            },
          },
          {
            label: 'Rename',
            run: () => {
              handlers.startRename(server);
            },
          },
          {
            label: 'Remove…',
            danger: true,
            run: () => {
              handlers.askRemove(server);
            },
          },
        ],
      }),
  );
}

function editingRow(server: Server, editing: Editing, handlers: ListHandlers): HTMLLIElement {
  return h(
    'li',
    { 'data-server-id': server.id, 'data-mode': 'editing' },
    h(
      'form',
      {
        class: 'editor',
        'aria-label': `Rename ${server.name}`,
        novalidate: true,
        onsubmit: (event) => {
          event.preventDefault();
          handlers.saveRename();
        },
        onkeydown: (event) => {
          if (event instanceof KeyboardEvent && event.key === 'Escape') {
            event.preventDefault();
            handlers.cancelRename();
          }
        },
      },
      h('input', {
        type: 'text',
        key: 'rename-input',
        value: editing.value,
        autocomplete: 'off',
        'aria-label': `Name for ${server.url}`,
        'aria-describedby': `rename-address-${server.id}`,
        'aria-invalid': editing.invalid ? 'true' : null,
        oninput: (event) => {
          if (event.target instanceof HTMLInputElement) handlers.editValue(event.target.value);
        },
      }),
      h('span', {
        class: 'url',
        id: `rename-address-${server.id}`,
        text: server.url,
        title: server.url,
      }),
      h(
        'div',
        { class: 'editor-actions' },
        h('button', {
          type: 'button',
          class: 'quiet',
          text: 'Cancel',
          onclick: () => {
            handlers.cancelRename();
          },
        }),
        h('button', {
          type: 'submit',
          class: 'primary',
          disabled: editing.saving,
          text: editing.saving ? 'Saving…' : 'Save',
        }),
      ),
    ),
  );
}

function confirmingRow(server: Server, view: ListView, handlers: ListHandlers): HTMLLIElement {
  const removing = view.removing === server.id;
  return h(
    'li',
    {
      class: 'confirm',
      'data-server-id': server.id,
      'data-mode': 'confirming',
      onkeydown: (event) => {
        if (event instanceof KeyboardEvent && event.key === 'Escape') {
          handlers.cancelRemove(server);
        }
      },
      onfocusout: (event) => {
        const row = event.currentTarget;
        const next = event instanceof FocusEvent ? event.relatedTarget : null;
        // WebKit does not focus a button on click, so clicking Remove fires focusout with a
        // null relatedTarget; disarming then would swallow that click.
        if (row instanceof Node && next instanceof Node && !row.contains(next)) {
          handlers.disarmRemove();
        }
      },
    },
    h('p', {
      text: `Remove ${server.name}? Its signed-in session and stored data are deleted too.`,
    }),
    h(
      'div',
      {},
      h('button', {
        type: 'button',
        class: 'quiet',
        key: `confirm-cancel-${server.id}`,
        disabled: removing,
        text: 'Cancel',
        onclick: () => {
          handlers.cancelRemove(server);
        },
      }),
      h('button', {
        type: 'button',
        class: 'danger',
        disabled: removing,
        text: removing ? 'Removing…' : 'Remove',
        onclick: () => {
          handlers.remove(server);
        },
      }),
    ),
  );
}

function rowFor(server: Server, view: ListView, handlers: ListHandlers): HTMLLIElement {
  if (view.editing?.id === server.id) return editingRow(server, view.editing, handlers);
  if (view.confirming === server.id) return confirmingRow(server, view, handlers);
  return normalRow(server, view, handlers);
}

export function renderServerList(
  list: HTMLElement,
  emptyState: HTMLElement,
  view: ListView,
  handlers: ListHandlers,
  options: RenderOptions = {},
): void {
  const liveEditor =
    options.reuseEditor === true && view.editing !== null
      ? findRow(list, view.editing.id, 'editing')
      : null;
  list.replaceChildren(
    ...view.servers.map((server) =>
      view.editing?.id === server.id && liveEditor !== null
        ? liveEditor
        : rowFor(server, view, handlers),
    ),
  );
  emptyState.hidden = view.servers.length > 0;
}

export function findRow(list: HTMLElement, serverId: string, mode?: string): HTMLElement | null {
  for (const row of list.children) {
    if (
      row instanceof HTMLElement &&
      row.dataset.serverId === serverId &&
      (mode === undefined || row.dataset.mode === mode)
    ) {
      return row;
    }
  }
  return null;
}

export function markFlagged(list: HTMLElement, flagged: ReadonlySet<string>): void {
  for (const row of list.children) {
    if (row instanceof HTMLElement) {
      row.toggleAttribute('data-flagged', flagged.has(row.dataset.serverId ?? ''));
    }
  }
}
