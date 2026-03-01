import { logger } from '../utils/logger.js';
import { formatDate } from '../utils/date-format.js';

export function initialize() {
  logger.info(formatDate());
}
