import { describe, expect, it } from 'vitest';
import cases from '../fixtures/server-url-cases.json' with { type: 'json' };
import { checkServerUrl } from './server-url';

describe('checkServerUrl', () => {
  it.each(cases)('$input', ({ input, url, error }) => {
    const expected = url === undefined ? { ok: false, error } : { ok: true, url };

    expect(checkServerUrl(input)).toEqual(expected);
  });
});
