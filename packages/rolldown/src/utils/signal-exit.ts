import { onExit as originalOnExit } from 'signal-exit';

export function onExit(...args: Parameters<typeof originalOnExit>): void {
  // process is undefined for browser build
  if (typeof process === 'object' && process.versions.webcontainer) {
    // signal-exit does not work properly in webcontainers
    // (https://github.com/rolldown/rolldown/issues/7381)
    process.on('exit', (code) => {
      args[0](code, null);
    });
    return;
  }

  originalOnExit(...args);
}
