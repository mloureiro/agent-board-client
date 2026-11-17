export type ServerUrlError = 'invalid' | 'insecure' | 'credentials';

export type ServerUrlResult =
  | { readonly ok: true; readonly url: string }
  | { readonly ok: false; readonly error: ServerUrlError };

const PLAIN_HTTP_HOSTS: ReadonlySet<string> = new Set(['localhost', '127.0.0.1', '[::1]']);

export function checkServerUrl(input: string): ServerUrlResult {
  const trimmed = input.trim();
  if (/\s/.test(trimmed) || !URL.canParse(trimmed)) return { ok: false, error: 'invalid' };

  const url = new URL(trimmed);
  const isHttps = url.protocol === 'https:';
  const isHttp = url.protocol === 'http:';
  if ((!isHttps && !isHttp) || url.hostname === '') return { ok: false, error: 'invalid' };

  if (url.username !== '' || url.password !== '') return { ok: false, error: 'credentials' };

  if (isHttp && !PLAIN_HTTP_HOSTS.has(url.hostname)) return { ok: false, error: 'insecure' };

  return { ok: true, url: url.href };
}
