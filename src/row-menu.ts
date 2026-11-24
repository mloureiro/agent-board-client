import { h, icon } from './dom';

export function nextMenuIndex(key: string, index: number, count: number): number {
  switch (key) {
    case 'ArrowDown':
      return (index + 1) % count;
    case 'ArrowUp':
      return (index - 1 + count) % count;
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return index;
  }
}

const NAVIGATION_KEYS: ReadonlySet<string> = new Set(['ArrowDown', 'ArrowUp', 'Home', 'End']);

export interface MenuButtonOptions {
  readonly id: string;
  readonly label: string;
  readonly expanded: boolean;
  readonly disabled: boolean;
  readonly onOpen: (at: 'first' | 'last') => void;
  readonly onClose: () => void;
}

export function menuButton(options: MenuButtonOptions): HTMLButtonElement {
  return h(
    'button',
    {
      type: 'button',
      class: 'more',
      key: `menu-${options.id}`,
      id: `menu-button-${options.id}`,
      'data-menu': options.id,
      disabled: options.disabled,
      'aria-haspopup': 'menu',
      'aria-expanded': String(options.expanded),
      'aria-controls': options.expanded ? `menu-list-${options.id}` : null,
      'aria-label': options.label,
      onclick: () => {
        if (options.expanded) options.onClose();
        else options.onOpen('first');
      },
      onkeydown: (event) => {
        if (!(event instanceof KeyboardEvent)) return;
        if (event.key === 'ArrowDown') {
          event.preventDefault();
          options.onOpen('first');
        } else if (event.key === 'ArrowUp') {
          event.preventDefault();
          options.onOpen('last');
        }
      },
    },
    icon('more'),
  );
}

export interface MenuItem {
  readonly label: string;
  readonly danger?: boolean;
  readonly run: () => void;
}

export interface MenuOptions {
  readonly id: string;
  readonly items: readonly MenuItem[];
  readonly onClose: () => void;
}

export function menu(options: MenuOptions): HTMLUListElement {
  const list = h(
    'ul',
    {
      class: 'menu',
      role: 'menu',
      id: `menu-list-${options.id}`,
      'data-menu': options.id,
      'aria-labelledby': `menu-button-${options.id}`,
      onkeydown: (event) => {
        if (!(event instanceof KeyboardEvent)) return;
        if (event.key === 'Escape' || event.key === 'Tab') {
          if (event.key === 'Escape') event.preventDefault();
          options.onClose();
          return;
        }
        if (!NAVIGATION_KEYS.has(event.key)) return;
        event.preventDefault();
        const items = [...list.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
        const index = items.findIndex((item) => item === document.activeElement);
        items[nextMenuIndex(event.key, index, items.length)]?.focus();
      },
    },
    ...options.items.map((item, index) =>
      h(
        'li',
        { role: 'none' },
        h('button', {
          type: 'button',
          role: 'menuitem',
          tabindex: '-1',
          class: item.danger === true ? 'danger-item' : null,
          key: `item-${options.id}-${String(index)}`,
          text: item.label,
          onclick: item.run,
        }),
      ),
    ),
  );
  return list;
}
