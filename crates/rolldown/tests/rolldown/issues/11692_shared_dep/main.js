import { Heavy, LiveLazy } from './index.js';

globalThis.__ready = LiveLazy().then((mod) => ({
  heavy: Heavy(),
  live: mod.runLive(),
}));
