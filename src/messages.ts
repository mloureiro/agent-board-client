import { isBackendError } from './backend';
import type { ServerUrlError } from './server-url';

const URL_MESSAGES: Record<ServerUrlError, string> = {
  invalid: 'Enter the full address of the board, starting with https://',
  insecure: 'Use https. Plain http is accepted only for localhost, 127.0.0.1 and [::1].',
  credentials: 'Remove the user name and password from the address. Sign-in happens on the board.',
};

export function urlMessage(error: ServerUrlError): string {
  return URL_MESSAGES[error];
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
    case 'storedData':
      return `The server was kept, because its stored data could not be deleted: ${failure.message}`;
  }
}
