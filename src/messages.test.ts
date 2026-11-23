import { describe, expect, it } from 'vitest';
import {
  removeFailureMessage,
  renameSavedMessage,
  subjectMessage,
  testResultMessage,
} from './messages';

describe('renameSavedMessage', () => {
  it('reports the saved name and the title error for a window failure', () => {
    const message = renameSavedMessage({ kind: 'window', message: 'window not found' });

    expect(message).toContain('window not found');
    expect(message).toContain('name was saved');
    expect(message).not.toMatch(/open/i);
  });

  it.each([
    { kind: 'name' },
    { kind: 'notFound' },
    { kind: 'storage', message: 'denied' },
    { kind: 'url', reason: 'invalid' },
    { kind: 'connection', reason: { kind: 'unreachable' } },
    'not a backend error',
  ])('returns null for %j', (failure) => {
    expect(renameSavedMessage(failure)).toBeNull();
  });
});

describe('removeFailureMessage', () => {
  it('says the window could not be closed for a window failure', () => {
    expect(removeFailureMessage('Team', { kind: 'window', message: 'gone' })).toBe(
      'Team was removed, but its window could not be closed: gone',
    );
  });

  it('says the server could not be removed for a storage failure', () => {
    expect(removeFailureMessage('Team', { kind: 'storage', message: 'denied' })).toBe(
      'Team could not be removed. The server list could not be saved: denied',
    );
  });
});

describe('testResultMessage', () => {
  it('says the server answered when there is no failure', () => {
    expect(testResultMessage('Team', null)).toBe('Team answered. It can be opened.');
  });

  it('says the server cannot be reached with the connection reason', () => {
    expect(testResultMessage('Team', { kind: 'connection', reason: { kind: 'timeout' } })).toBe(
      "Team can't be reached. The server did not answer in time.",
    );
  });
});

describe('subjectMessage', () => {
  it('lowers the first letter of the text', () => {
    expect(subjectMessage('Team', 'Give the server a name.')).toBe('Team: give the server a name.');
  });
});
