import type { WatchOptions } from '../options/watch-options';
import { arraify } from './misc';

const CLEAR_SCREEN = '\x1Bc';

export function getClearScreenFunction(
  options: WatchOptions | WatchOptions[],
): (() => void) | undefined {
  const isTTY = process.stdout.isTTY;
  const isAnyOptionNotAllowingClearScreen = arraify(options).some(
    ({ watch }) => watch === false || watch?.clearScreen === false,
  );

  if (isTTY && !isAnyOptionNotAllowingClearScreen) {
    return () => {
      process.stdout.write(CLEAR_SCREEN);
    };
  }
}
