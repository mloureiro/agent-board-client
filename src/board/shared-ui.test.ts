import {
  MIN_UI_SERVER,
  UI_API_VERSION,
  UI_SERVER_FEATURES,
  parseUiApiHeader,
} from '@agent-board/shared/ui';
import { expect, it } from 'vitest';

import uiApi from '../../fixtures/ui-api.json';

it('loads the shared contract source through the client toolchain', () => {
  expect(parseUiApiHeader(String(UI_API_VERSION))).toBe(UI_API_VERSION);
});

it('keeps the numbers the native side reads equal to the shared contract', () => {
  expect(uiApi.uiApiVersion).toBe(UI_API_VERSION);
  expect(uiApi.minUiServer).toBe(MIN_UI_SERVER);
  expect(UI_SERVER_FEATURES).toContain(uiApi.eventsFeature);
});
