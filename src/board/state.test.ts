import { describe, expect, it } from 'vitest';

import { refetchIntervalFor, SLOW_POLL_MS } from './state.ts';

describe('refetchIntervalFor', () => {
  it.each([
    [true, false],
    [false, SLOW_POLL_MS],
  ])('with events up %s polls %s', (eventsUp, expected) => {
    expect(refetchIntervalFor(eventsUp)).toBe(expected);
  });
});
