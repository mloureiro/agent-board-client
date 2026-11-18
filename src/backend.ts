import { invoke } from '@tauri-apps/api/core';
import type { ServerUrlError } from './server-url';

export interface Server {
  readonly id: string;
  readonly name: string;
  readonly url: string;
  readonly notifications: { readonly enabled: boolean };
}

export type BackendError =
  | { readonly kind: 'name' }
  | { readonly kind: 'url'; readonly reason: ServerUrlError }
  | { readonly kind: 'notFound' }
  | { readonly kind: 'storage'; readonly message: string }
  | { readonly kind: 'window'; readonly message: string }
  | { readonly kind: 'storedData'; readonly message: string };

const BACKEND_ERROR_KINDS: ReadonlySet<string> = new Set([
  'name',
  'url',
  'notFound',
  'storage',
  'window',
  'storedData',
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

export function removeServer(id: string, deleteData: boolean): Promise<void> {
  return invoke('remove_server', { id, deleteData });
}

export function setServerNotifications(id: string, enabled: boolean): Promise<Server> {
  return invoke<Server>('set_server_notifications', { id, enabled });
}

export function openServer(id: string): Promise<void> {
  return invoke('open_server', { id });
}
