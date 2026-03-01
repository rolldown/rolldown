import { logger } from '../utils/logger.js';
import { validate } from '../utils/validator.js';

export function render(data) {
  if (validate(data)) logger.info();
}
