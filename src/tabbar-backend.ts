import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';

export interface TabView {
  readonly id: number;
  readonly label: string;
  readonly home: boolean;
}

export interface TabsView {
  readonly selected: number;
  readonly tabs: readonly TabView[];
}

export function tabsSnapshot(): Promise<TabsView> {
  return invoke<TabsView>('tabs_snapshot');
}

export function selectTab(tab: number): Promise<void> {
  return invoke('select_tab', { tab });
}

export function closeTab(tab: number): Promise<void> {
  return invoke('close_tab', { tab });
}

// Listening through the webview registers a webview-scoped listener; a plain `listen` would
// also receive events emitted to another server window's tab bar.
export function onTabsChanged(handler: (view: TabsView) => void): Promise<void> {
  return getCurrentWebview()
    .listen<TabsView>('tabs-changed', (event) => {
      handler(event.payload);
    })
    .then(() => undefined);
}
