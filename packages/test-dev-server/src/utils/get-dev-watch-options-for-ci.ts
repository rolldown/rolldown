import { DevWatchOptions } from 'rolldown/experimental';

export function getDevWatchOptionsForCi() {
  return {
    usePolling: true,
    pollInterval: 50,
    compareContentsForPolling: false,
    useDebounce: true,
    debounceDuration: 310,
    debounceTickRate: 300,
  } satisfies DevWatchOptions;
}
