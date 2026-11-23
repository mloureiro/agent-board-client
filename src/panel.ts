import type { FailedDeletion } from './backend';
import { icon } from './dom';
import { storeDeletionMessage } from './messages';
import type { Notice } from './notices';

export interface PanelHandlers {
  readonly dismiss: (notice: Notice) => void;
  readonly retry: (failure: FailedDeletion) => void;
}

function entry(tone: 'ok' | 'error', text: string, action: HTMLButtonElement): HTMLLIElement {
  const message = document.createElement('p');
  message.textContent = text;

  const item = document.createElement('li');
  item.dataset.tone = tone;
  item.append(icon(tone), message, action);
  return item;
}

function actionButton(label: string, className: string, onClick: () => void): HTMLButtonElement {
  const element = document.createElement('button');
  element.type = 'button';
  element.className = className;
  element.textContent = label;
  element.addEventListener('click', onClick);
  return element;
}

function noticeEntry(notice: Notice, handlers: PanelHandlers): HTMLLIElement {
  const item = entry(
    notice.tone,
    notice.text,
    actionButton('Dismiss', 'quiet', () => {
      handlers.dismiss(notice);
    }),
  );
  item.dataset.noticeId = String(notice.id);
  return item;
}

function deletionEntry(
  failure: FailedDeletion,
  retrying: boolean,
  handlers: PanelHandlers,
): HTMLLIElement {
  const retry = actionButton(retrying ? 'Retrying…' : 'Retry', 'primary', () => {
    handlers.retry(failure);
  });
  retry.disabled = retrying;

  const item = entry('error', storeDeletionMessage(failure.name), retry);
  item.dataset.deletionId = failure.id;
  return item;
}

export function renderPanel(
  container: HTMLElement,
  notices: readonly Notice[],
  deletions: readonly FailedDeletion[],
  retrying: ReadonlySet<string>,
  handlers: PanelHandlers,
): void {
  const entries = [
    ...notices.map((notice) => noticeEntry(notice, handlers)).reverse(),
    ...deletions.map((failure) => deletionEntry(failure, retrying.has(failure.id), handlers)),
  ];
  container.replaceChildren(...entries);
  container.hidden = entries.length === 0;
}
