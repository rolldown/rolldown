import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
export const CONFIG = {
  watch: getDevWatchOptionsForCi(),
};
