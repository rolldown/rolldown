import { early } from './c.js';

await new Promise((resolve) => setTimeout(resolve, 10));
export const seenAfterPause = early.length;
