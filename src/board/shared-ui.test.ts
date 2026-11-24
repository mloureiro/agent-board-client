import { UI_API_VERSION, parseUiApiHeader } from '@agent-board/shared/ui';
import { expect, it } from 'vitest';

it('loads the shared contract source through the client toolchain', () => {
  expect(parseUiApiHeader(String(UI_API_VERSION))).toBe(UI_API_VERSION);
});
