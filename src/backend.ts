import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ServerUrlError } from './server-url';

export interface Server {
  readonly id: string;
  readonly name: string;
  readonly url: string;
}

export type BackendError =
  | { readonly kind: 'name' }
  | { readonly kind: 'url'; readonly reason: ServerUrlError }
  | { readonly kind: 'notFound' }
  | { readonly kind: 'storage'; readonly message: string }
  | { readonly kind: 'window'; readonly message: string };

const BACKEND_ERROR_KINDS: ReadonlySet<string> = new Set([
  'name',
  'url',
  'notFound',
  'storage',
  'window',
]);

export function isBackendError(value: unknown): value is BackendError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    typeof value.kind === 'string' &&
    BACKEND_ERROR_KINDS.has(value.kind)
  );
}

export function listServers(): Promise<Server[]> {
  return invoke<Server[]>('list_servers');
}

export function addServer(name: string, url: string): Promise<Server> {
  return invoke<Server>('add_server', { name, url });
}

export function renameServer(id: string, name: string): Promise<Server> {
  return invoke<Server>('rename_server', { id, name });
}

export function removeServer(id: string): Promise<void> {
  return invoke('remove_server', { id });
}

export function openServer(id: string): Promise<void> {
  return invoke('open_server', { id });
}

export interface FailedDeletion {
  readonly id: string;
  readonly name: string | null;
}

export function failedStoreDeletions(): Promise<FailedDeletion[]> {
  return invoke<FailedDeletion[]>('failed_store_deletions');
}

export function retryStoreDeletion(id: string): Promise<void> {
  return invoke('retry_store_deletion', { id });
}

export function onStoreDeletionFailed(handler: (failure: FailedDeletion) => void): Promise<void> {
  return listen<FailedDeletion>('store-deletion-failed', (event) => {
    handler(event.payload);
  }).then(() => undefined);
}

export function onStoreDeletionSucceeded(handler: (id: string) => void): Promise<void> {
  return listen<string>('store-deletion-succeeded', (event) => {
    handler(event.payload);
  }).then(() => undefined);
}
