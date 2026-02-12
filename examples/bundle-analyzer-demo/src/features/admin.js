import { logger } from '../utils/logger.js';
import { formatDate } from '../utils/helpers.js';

export function initialize() {
  logger.info(formatDate());
}
