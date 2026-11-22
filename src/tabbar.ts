import { closeTab, onTabsChanged, selectTab, tabsSnapshot } from './tabbar-backend';
import type { TabsView } from './tabbar-backend';

const container = document.getElementById('tabs');

function render(view: TabsView): void {
  const buttons = view.tabs.map((tab) => {
    const group = document.createElement('span');

    const select = document.createElement('button');
    select.type = 'button';
    select.textContent = tab.home ? '🏠' : tab.label;
    select.disabled = tab.id === view.selected;
    select.addEventListener('click', () => {
      void selectTab(tab.id);
    });
    group.append(select);

    if (!tab.home) {
      const close = document.createElement('button');
      close.type = 'button';
      close.textContent = '×';
      close.addEventListener('click', () => {
        void closeTab(tab.id);
      });
      group.append(close);
    }
    return group;
  });
  container?.replaceChildren(...buttons);
}

await onTabsChanged(render);
render(await tabsSnapshot());
