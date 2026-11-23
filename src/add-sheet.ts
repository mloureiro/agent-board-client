import { addServer, isBackendError, testAddress } from './backend';
import type { Server } from './backend';
import { h, icon } from './dom';
import { failureMessage, testResultMessage, urlMessage } from './messages';
import { checkServerUrl } from './server-url';

export interface AddSheetOptions {
  readonly trigger: HTMLElement;
  readonly onAdded: (server: Server) => Promise<void>;
}

export interface AddSheet {
  readonly open: () => void;
}

function find<T extends HTMLElement>(root: HTMLElement, selector: string, type: new () => T): T {
  const found = root.querySelector(selector);
  if (!(found instanceof type)) throw new Error(`add sheet has no ${selector}`);
  return found;
}

export function createAddSheet(dialog: HTMLDialogElement, options: AddSheetOptions): AddSheet {
  const form = find(dialog, 'form', HTMLFormElement);
  const nameInput = find(dialog, '#server-name', HTMLInputElement);
  const urlInput = find(dialog, '#server-url', HTMLInputElement);
  const nameMessage = find(dialog, '#name-message', HTMLElement);
  const urlMessageElement = find(dialog, '#url-message', HTMLElement);
  const status = find(dialog, '#sheet-status', HTMLElement);
  const cancelButton = find(dialog, '#sheet-cancel', HTMLButtonElement);
  const testButton = find(dialog, '#sheet-test', HTMLButtonElement);
  const addButton = find(dialog, '#sheet-add', HTMLButtonElement);

  let testRequest = 0;
  let adding = false;

  function showField(input: HTMLInputElement, message: HTMLElement, text: string | null): void {
    message.replaceChildren(...(text === null ? [] : [icon('error'), h('span', { text })]));
    if (text === null) input.removeAttribute('aria-invalid');
    else input.setAttribute('aria-invalid', 'true');
  }

  function showStatus(tone: 'ok' | 'error' | null, text = ''): void {
    status.dataset.tone = tone ?? '';
    status.replaceChildren(...(tone === null ? [] : [icon(tone), h('span', { text })]));
  }

  function clearMessages(): void {
    showField(nameInput, nameMessage, null);
    showField(urlInput, urlMessageElement, null);
    showStatus(null);
  }

  function setTesting(testing: boolean): void {
    testButton.disabled = testing;
    testButton.textContent = testing ? 'Testing…' : 'Test';
  }

  function reset(): void {
    testRequest += 1;
    adding = false;
    form.reset();
    clearMessages();
    setTesting(false);
    addButton.disabled = false;
    addButton.textContent = 'Add server';
  }

  function close(): void {
    reset();
    dialog.close();
    options.trigger.focus();
  }

  function checkedAddress(): string | null {
    const checked = checkServerUrl(urlInput.value);
    if (checked.ok) return checked.url;
    showField(urlInput, urlMessageElement, urlMessage(checked.error));
    urlInput.focus();
    return null;
  }

  async function runTest(): Promise<void> {
    clearMessages();
    const url = checkedAddress();
    if (url === null) return;

    testRequest += 1;
    const request = testRequest;
    setTesting(true);
    const name = nameInput.value.trim();
    const label = name === '' ? 'This address' : name;
    const failure = await testAddress(url).then(
      () => null,
      (error: unknown) => ({ error }),
    );
    if (request !== testRequest) return;
    setTesting(false);
    if (failure === null) showStatus('ok', testResultMessage(label, null));
    else showStatus('error', testResultMessage(label, failure.error));
  }

  async function runAdd(): Promise<void> {
    clearMessages();
    const url = checkedAddress();
    if (url === null || adding) return;

    adding = true;
    addButton.disabled = true;
    addButton.textContent = 'Adding…';
    let added: Server;
    try {
      added = await addServer(nameInput.value, url);
    } catch (failure) {
      adding = false;
      addButton.disabled = false;
      addButton.textContent = 'Add server';
      if (isBackendError(failure) && failure.kind === 'name') {
        showField(nameInput, nameMessage, failureMessage(failure));
        nameInput.focus();
      } else {
        showStatus('error', failureMessage(failure));
      }
      return;
    }
    close();
    await options.onAdded(added);
  }

  urlInput.addEventListener('input', () => {
    testRequest += 1;
    setTesting(false);
    showStatus(null);
  });
  cancelButton.addEventListener('click', close);
  testButton.addEventListener('click', () => {
    runTest().catch((failure: unknown) => {
      showStatus('error', failureMessage(failure));
    });
  });
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    runAdd().catch((failure: unknown) => {
      showStatus('error', failureMessage(failure));
    });
  });
  dialog.addEventListener('cancel', (event) => {
    event.preventDefault();
    close();
  });
  dialog.addEventListener('click', (event) => {
    if (event.target === dialog) close();
  });

  return {
    open: () => {
      reset();
      dialog.showModal();
      nameInput.focus();
    },
  };
}
