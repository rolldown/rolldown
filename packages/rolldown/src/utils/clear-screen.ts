import type { RolldownOptions } from '..';
import { arraify } from './misc';

const CLEAR_SCREEN = '\x1Bc';

const noop = (): void => {};

export function getClearScreenFunction(
  rolldownOptions: RolldownOptions | RolldownOptions[],
): () => void {
  const isTTY = process.stdout.isTTY;

  let isAnyOptionNotAllowingClearScreen = arraify(rolldownOptions)
    .map(
      (config) => {
        if (typeof config.watch === 'boolean') {
          return config.watch ? true : false;
        }

        // Default value for `watch.clearScreen` is `true`.
        return config.watch?.clearScreen ?? true;
      },
    )
    .some(
      (clearScreen) => clearScreen === false,
    );

  const shouldClearScreen = isTTY && !isAnyOptionNotAllowingClearScreen;

  if (!shouldClearScreen) {
    return noop;
  }

  return () => {
    process.stdout.write(CLEAR_SCREEN);
  };
}
