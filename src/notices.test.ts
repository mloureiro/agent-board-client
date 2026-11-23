import { describe, expect, it } from 'vitest';
import { clearFor, flaggedServers } from './notices';
import type { Notice, NoticeAction, NoticeCause } from './notices';

const CAUSES: readonly NoticeCause[] = [
  'test',
  'open',
  'openedLink',
  'remove',
  'rename',
  'retry',
  'add',
  'startup',
];

const notices: readonly Notice[] = CAUSES.map((cause, id) => ({
  id,
  cause,
  serverId: null,
  tone: 'error',
  text: cause,
}));

describe('clearFor', () => {
  const statusOnly: readonly NoticeCause[] = ['test', 'open', 'openedLink'];

  it.each<[NoticeAction, readonly NoticeCause[]]>([
    ['test', []],
    ['open', []],
    ['remove', statusOnly],
    ['rename', statusOnly],
    ['cancelRename', statusOnly],
    ['retry', statusOnly],
    ['add', statusOnly],
  ])('%s keeps %j', (action, surviving) => {
    expect(clearFor(notices, action).map((notice) => notice.cause)).toEqual(surviving);
  });
});

describe('flaggedServers', () => {
  it('keeps error notices that name a server', () => {
    const flagged = flaggedServers([
      { id: 1, cause: 'test', serverId: 'a', tone: 'error', text: '' },
      { id: 2, cause: 'test', serverId: 'b', tone: 'ok', text: '' },
      { id: 3, cause: 'remove', serverId: null, tone: 'error', text: '' },
    ]);

    expect([...flagged]).toEqual(['a']);
  });
});
