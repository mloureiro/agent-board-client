import { runInNewContext } from 'node:vm';
import { describe, expect, it, vi } from 'vitest';
import script from './board-notifications.js?raw';

interface PageNotification {
  onclick: (() => void) | null;
  onerror: (() => void) | null;
  close(): void;
}

interface Page {
  Notification: {
    new (title: string, options?: { body?: string; tag?: string }): PageNotification;
    readonly permission: string;
    requestPermission(): Promise<string>;
  };
  __agentBoardClient: { activateNotification(key: string): void };
}

type Invoke = (command: string, payload: { title: string; body: string; tag: string }) => unknown;

function loadPage(invoke: Invoke = () => Promise.resolve(null)) {
  const invoked = vi.fn(invoke);
  const window = { __TAURI_INTERNALS__: { invoke: invoked } };
  runInNewContext(script, { window, EventTarget, Event });
  return { page: window as unknown as Page, invoked };
}

describe('the Notification a board page gets', () => {
  it('reports permission as granted without asking the system', async () => {
    const { page, invoked } = loadPage();

    expect(page.Notification.permission).toBe('granted');
    await expect(page.Notification.requestPermission()).resolves.toBe('granted');
    expect(invoked).not.toHaveBeenCalled();
  });

  it('hands a new notification to the notify command', () => {
    const { page, invoked } = loadPage();

    new page.Notification('Fix the build', { body: 'Needs you', tag: 'task-1' });

    expect(invoked).toHaveBeenCalledExactlyOnceWith('notify', {
      title: 'Fix the build',
      body: 'Needs you',
      tag: 'task-1',
    });
  });

  it('gives untagged notifications separate keys', () => {
    const { page, invoked } = loadPage();

    new page.Notification('First');
    new page.Notification('Second');

    const [first, second] = invoked.mock.calls.map(([, payload]) => payload.tag);
    expect(first).not.toBe('');
    expect(first).not.toBe(second);
  });

  it('runs the click handler of the activated notification only', () => {
    const { page } = loadPage();
    const first = new page.Notification('First', { tag: 'task-1' });
    const second = new page.Notification('Second', { tag: 'task-2' });
    first.onclick = vi.fn();
    second.onclick = vi.fn();

    page.__agentBoardClient.activateNotification('task-2');

    expect(first.onclick).not.toHaveBeenCalled();
    expect(second.onclick).toHaveBeenCalledOnce();
  });

  it('does not run the click handler of a closed notification', () => {
    const { page } = loadPage();
    const notification = new page.Notification('First', { tag: 'task-1' });
    notification.onclick = vi.fn();

    notification.close();
    page.__agentBoardClient.activateNotification('task-1');

    expect(notification.onclick).not.toHaveBeenCalled();
  });

  it('runs the error handler when the client refuses the notification', async () => {
    const { page } = loadPage(() => Promise.reject(new Error('refused')));
    const notification = new page.Notification('First', { tag: 'task-1' });
    const failed = new Promise<void>((resolve) => {
      notification.onerror = () => {
        resolve();
      };
    });

    await expect(failed).resolves.toBeUndefined();
  });
});
