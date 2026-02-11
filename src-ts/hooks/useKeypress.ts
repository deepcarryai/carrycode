/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { useRef } from 'react';
import { useInput } from 'ink';

export interface Key {
  name: string;
  ctrl?: boolean;
  meta?: boolean;
  shift?: boolean;
  sequence?: string;
}

export type KeypressHandler = (key: Key) => void;

export function useKeypress(
  onKeypress: KeypressHandler,
  { isActive }: { isActive: boolean },
) {
  const lastEscapeTime = useRef(0);
  const lastBracketTime = useRef(0);

  useInput(
    (input, key) => {
      if (!isActive) return;

      // Filter out Focus In/Out sequences (\x1b[I and \x1b[O)
      // These can appear when terminal focus tracking is enabled
      
      // Case 1: Sequence received in a single batch
      if (input.includes('\x1b[O') || input.includes('\x1b[I')) {
        input = input.replace(/\x1b\[[OI]/g, '');
        if (input.length === 0) return;
      }

      // Case 2: Sequence split into multiple events
      const now = Date.now();
      
      // Don't filter if modifier keys are pressed (e.g. Alt+[)
      if (!key.ctrl && !key.meta) {
        if (key.escape && input === '\x1b') {
          lastEscapeTime.current = now;
        } else if (input === '[' && (now - lastEscapeTime.current < 50)) {
          lastBracketTime.current = now;
          return;
        } else if ((input === 'O' || input === 'I') && (now - lastBracketTime.current < 50)) {
          return;
        }
      }

      let name = input;

      if (key.return) name = 'return';
      else if (key.escape) name = 'escape';
      else if (key.leftArrow) name = 'left';
      else if (key.rightArrow) name = 'right';
      else if (key.upArrow) name = 'up';
      else if (key.downArrow) name = 'down';
      else if (key.backspace || key.delete || input === '\x08' || input === '\x7f') name = 'backspace';
      else if (key.tab) name = 'tab';
      else if (key.pageDown) name = 'pagedown';
      else if (key.pageUp) name = 'pageup';

      onKeypress({
        name,
        ctrl: key.ctrl,
        meta: key.meta,
        shift: key.shift,
        sequence: input,
      });
    },
    { isActive },
  );
}
