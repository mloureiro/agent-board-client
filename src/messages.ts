import { isBackendError } from './backend';
import type { ConnectionFailure } from './backend';
import type { ServerUrlError } from './server-url';

const URL_MESSAGES: Record<ServerUrlError, string> = {
  invalid: 'Enter the full address of the board, starting with https://',
  insecure: 'Use https. Plain http is accepted only for localhost, 127.0.0.1 and [::1].',
  credentials: 'Remove the user name and password from the address. Sign-in happens on the board.',
};

export function urlMessage(error: ServerUrlError): string {
  return URL_MESSAGES[error];
}

function connectionMessage(failure: ConnectionFailure): string {
  switch (failure.kind) {
    case 'unreachable':
      return 'Nothing answered at this address. Check that the server is running.';
    case 'timeout':
      return 'The server did not answer in time.';
    case 'status':
      return `The server answered with an error (HTTP ${String(failure.status)}).`;
    case 'other':
      return `The connection failed: ${failure.message}`;
  }
}

export function failureMessage(failure: unknown): string {
  if (!isBackendError(failure)) return 'Something went wrong. Try again.';

  switch (failure.kind) {
    case 'name':
      return 'Give the server a name.';
    case 'url':
      return urlMessage(failure.reason);
    case 'notFound':
      return 'That server is no longer in the list.';
    case 'storage':
      return `The server list could not be saved: ${failure.message}`;
    case 'window':
      return `The server could not be opened: ${failure.message}`;
    case 'connection':
      return connectionMessage(failure.reason);
  }
}

export function storeDeletionMessage(name: string | null): string {
  return `Couldn't delete the stored data of ${name ?? 'a removed server'}.`;
}

export function testResultMessage(label: string, failure: unknown): string {
  if (failure === null) return `${label} answered. It can be opened.`;
  return `${label} can't be reached. ${failureMessage(failure)}`;
}

export function openFailureMessage(name: string, failure: unknown): string {
  return `${name} couldn't be opened. ${failureMessage(failure)}`;
}

export function removeFailureMessage(name: string, failure: unknown): string {
  if (isBackendError(failure) && failure.kind === 'window') {
    return `${name} was removed, but its window could not be closed: ${failure.message}`;
  }
  return `${name} could not be removed. ${failureMessage(failure)}`;
}

export function renameSavedMessage(failure: unknown): string | null {
  if (!isBackendError(failure) || failure.kind !== 'window') return null;
  return `The name was saved, but the window title could not be updated: ${failure.message}`;
}

export function subjectMessage(name: string, text: string): string {
  return `${name}: ${text.charAt(0).toLowerCase()}${text.slice(1)}`;
}
