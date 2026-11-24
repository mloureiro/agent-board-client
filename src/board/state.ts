import { useQuery, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useEffect, useState } from 'react';

export type BoardStateName =
  | 'ready'
  | 'signed-out'
  | 'board-only'
  | 'board-too-old'
  | 'upgrade-required'
  | 'offline'
  | 'server';

export interface BoardStateView {
  readonly state: BoardStateName;
  readonly serverName: string;
  readonly eventsUp: boolean;
  readonly signedIn: boolean;
}

export const SLOW_POLL_MS = 30_000;
export const BOARD_STATE_KEY = ['board-state'] as const;

export function refetchIntervalFor(eventsUp: boolean): number | false {
  return eventsUp ? false : SLOW_POLL_MS;
}

export function useBoardState(): {
  view: BoardStateView | undefined;
  isError: boolean;
  refetchInterval: number | false;
} {
  const queryClient = useQueryClient();
  const [listening, setListening] = useState(false);
  const { data: view, isError } = useQuery({
    queryKey: BOARD_STATE_KEY,
    queryFn: () => invoke<BoardStateView>('board_state'),
    enabled: listening,
  });
  const [pushedUp, setPushedUp] = useState<boolean | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void)[] = [];
    // A webview-scoped listener only receives events emitted to this webview; a plain `listen`
    // would also receive those of another server window.
    const webview = getCurrentWebview();
    Promise.all([
      webview.listen('board-changed', () => {
        void queryClient.invalidateQueries({
          predicate: (query) => query.queryKey[0] !== BOARD_STATE_KEY[0],
          refetchType: 'active',
        });
      }),
      webview.listen<{ up: boolean }>('board-events', (event) => {
        setPushedUp(event.payload.up);
      }),
      webview.listen('signed-out', () => {
        void queryClient.invalidateQueries({ queryKey: BOARD_STATE_KEY });
      }),
    ])
      .then((handlers) => {
        if (cancelled) {
          handlers.forEach((handler) => {
            handler();
          });
        } else {
          unlisten = handlers;
          setListening(true);
        }
      })
      .catch((error: unknown) => {
        console.error('The board listeners could not be registered', error);
        if (!cancelled) {
          setListening(true);
        }
      });

    return () => {
      cancelled = true;
      unlisten.forEach((handler) => {
        handler();
      });
    };
  }, [queryClient]);

  return {
    view,
    isError,
    refetchInterval: refetchIntervalFor(pushedUp ?? view?.eventsUp ?? false),
  };
}
