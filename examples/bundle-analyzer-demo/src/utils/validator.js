import { logger } from './logger.js';

export function validate(v) {
  if (!v) logger.warn();
  return !!v;
}
