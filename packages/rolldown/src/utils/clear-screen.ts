import type { RolldownOptions } from '..';
import { arraify } from './misc';

const CLEAR_SCREEN = '\x1Bc';

const noop = (): void => {};

export function getClearScreenFunction(
  rolldownOptions: RolldownOptions | RolldownOptions[],
): () => void {
  const isTTY = process.stdout.isTTY;

  let isAnyConfigContainsClearScreen = arraify(rolldownOptions)
    .map(
      (config) => {
        if (typeof config.watch === 'boolean') {
          return config.watch ? true : false; // Default value for `watch.clearScreen` is `true`.
        }

        return config.watch?.clearScreen ?? true;
      },
    )
    .some(
      (clearScreen) => clearScreen === true,
    );

  const shouldClearScreen = isTTY && isAnyConfigContainsClearScreen;

  if (!shouldClearScreen) {
    return noop;
  }

  return () => {
    process.stdout.write(CLEAR_SCREEN);
  };
}
