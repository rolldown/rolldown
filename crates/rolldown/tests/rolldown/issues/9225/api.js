import { getEnvInt } from './env.js';

export const api = getEnvInt('REQUEST_TIMEOUT_MS');
