import { RawData } from 'ws';

import { ClientMessage } from '../types/client-message';

export function decodeClientMessage(data: RawData): ClientMessage {
  const stringified = data.toString();
  const decoded = JSON.parse(stringified);
  if (decoded.type === 'hmr:invalidate') {
    return { type: 'hmr:invalidate', moduleId: decoded.moduleId };
  }

  throw new Error(`Unknown client message: ${stringified}`);
}
