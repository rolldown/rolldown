import { DevWatchOptions } from 'rolldown/experimental';

export function getDevWatchOptionsForCi(): DevWatchOptions {
  return {
    usePolling: true,
    pollInterval: 40,
    useDebounce: true,
    debounceDuration: 200,
    compareContentsForPolling: true,
  };
}
