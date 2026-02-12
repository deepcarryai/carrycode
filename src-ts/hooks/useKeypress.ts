/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

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
  useInput(
    (input, key) => {
      if (!isActive) return;

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
