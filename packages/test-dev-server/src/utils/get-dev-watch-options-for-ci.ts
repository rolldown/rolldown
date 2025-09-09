import { DevWatchOptions } from 'rolldown/experimental';

export function getDevWatchOptionsForCi(): DevWatchOptions {
  return {
    usePolling: true,
    pollInterval: 25,
    useDebounce: true,
    debounceDuration: 200,
  };
}
