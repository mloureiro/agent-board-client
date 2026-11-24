import { closeTab, onTabsChanged, selectTab, tabsSnapshot } from './tabbar-backend';
import type { Banner, TabsView } from './tabbar-backend';
import './tabbar.css';

const container = document.getElementById('tabs');
const banner = document.getElementById('banner');

const BANNER_COPY: Record<Banner, { lead: string; message: string }> = {
  unreachable: {
    lead: 'Not answering',
    message: 'This server is not answering. The board updates again when it is back.',
  },
  sessionExpired: {
    lead: 'Signed out',
    message: 'Your session has expired. Sign in again to keep the board up to date.',
  },
};

function renderBanner(kind: Banner | null): void {
  if (!banner || (banner.dataset.kind ?? null) === kind) return;
  if (kind === null) {
    banner.replaceChildren();
    banner.removeAttribute('data-kind');
    banner.hidden = true;
    return;
  }
  const copy = BANNER_COPY[kind];
  const lead = document.createElement('strong');
  lead.className = 'banner-lead';
  lead.textContent = copy.lead;
  const message = document.createElement('span');
  message.className = 'banner-message';
  message.textContent = copy.message;
  banner.replaceChildren(lead, message);
  banner.dataset.kind = kind;
  banner.hidden = false;
}

function logFailure(action: string): (error: unknown) => void {
  return (error) => {
    console.error(`${action} failed`, error);
  };
}

function render(view: TabsView): void {
  renderBanner(view.banner);
  if (!container) return;
  const focusedId = container.contains(document.activeElement)
    ? (document.activeElement as HTMLElement).closest<HTMLElement>('.tab')?.dataset.id
    : undefined;
  const previouslySelected =
    container.querySelector<HTMLElement>('.tab[data-selected]')?.dataset.id;

  const tabs = view.tabs.map((tab) => {
    const selected = tab.id === view.selected;
    const wrapper = document.createElement('div');
    wrapper.className = 'tab';
    wrapper.setAttribute('role', 'presentation');
    wrapper.dataset.id = String(tab.id);
    if (tab.home) wrapper.dataset.home = '';
    if (selected) wrapper.dataset.selected = '';
    wrapper.addEventListener('mousedown', (event) => {
      if (event.button === 1) event.preventDefault();
    });
    wrapper.addEventListener('auxclick', (event) => {
      if (event.button !== 1 || tab.home) return;
      event.preventDefault();
      closeTab(tab.id).catch(logFailure('close_tab'));
    });

    const select = document.createElement('button');
    select.type = 'button';
    select.className = 'tab-select';
    select.setAttribute('role', 'tab');
    select.setAttribute('aria-selected', String(selected));
    select.tabIndex = selected ? 0 : -1;
    if (tab.home) {
      select.textContent = '🏠';
      select.setAttribute('aria-label', 'Board');
      select.title = 'Board';
    } else {
      select.textContent = tab.label;
      select.title = tab.label;
    }
    select.addEventListener('click', () => {
      selectTab(tab.id).catch(logFailure('select_tab'));
    });
    wrapper.append(select);

    if (!tab.home) {
      const close = document.createElement('button');
      close.type = 'button';
      close.className = 'tab-close';
      close.tabIndex = -1;
      close.textContent = '×';
      close.setAttribute('aria-label', `Close ${tab.label}`);
      close.addEventListener('click', () => {
        closeTab(tab.id).catch(logFailure('close_tab'));
      });
      wrapper.append(close);
    }
    return wrapper;
  });
  container.replaceChildren(...tabs);

  const selectedTab = container.querySelector<HTMLElement>('.tab[data-selected]');
  if (focusedId !== undefined) {
    const restored = container.querySelector<HTMLElement>(
      `.tab[data-id="${focusedId}"] .tab-select`,
    );
    (restored ?? selectedTab?.querySelector<HTMLElement>('.tab-select'))?.focus();
  }
  if (selectedTab && selectedTab.dataset.id !== previouslySelected) {
    selectedTab.scrollIntoView({ inline: 'nearest', block: 'nearest' });
  }
}

function tabButtons(): HTMLElement[] {
  return Array.from(container?.querySelectorAll<HTMLElement>('.tab-select') ?? []);
}

container?.addEventListener('keydown', (event) => {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const tab = target.closest<HTMLElement>('.tab');
  const buttons = tabButtons();
  const current = buttons.findIndex((button) => button.closest('.tab') === tab);
  if (!tab || current < 0) return;

  const destination: Record<string, number> = {
    ArrowLeft: Math.max(current - 1, 0),
    ArrowRight: Math.min(current + 1, buttons.length - 1),
    Home: 0,
    End: buttons.length - 1,
  };
  const index = destination[event.key];
  if (index !== undefined) {
    event.preventDefault();
    buttons[index]?.focus();
    return;
  }

  if ((event.key === 'Delete' || event.key === 'Backspace') && !('home' in tab.dataset)) {
    event.preventDefault();
    closeTab(Number(tab.dataset.id)).catch(logFailure('close_tab'));
  }
});

container?.addEventListener(
  'wheel',
  (event) => {
    if (event.deltaX !== 0 || !(event.currentTarget instanceof HTMLElement)) return;
    event.currentTarget.scrollLeft += event.deltaY;
  },
  { passive: true },
);

const live = { eventRendered: false };
await onTabsChanged((view) => {
  live.eventRendered = true;
  render(view);
});
const snapshot = await tabsSnapshot();
if (!live.eventRendered) render(snapshot);
