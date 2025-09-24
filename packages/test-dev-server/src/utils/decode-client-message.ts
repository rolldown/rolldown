import { RawData } from 'ws';

import { ClientMessage } from '../types/client-message';

export function decodeClientMessage(data: RawData): ClientMessage {
  const stringified = data.toString();
  const decoded = JSON.parse(stringified) as ClientMessage;
  switch (decoded.type) {
    case 'hmr:invalidate':
      return { type: 'hmr:invalidate', moduleId: decoded.moduleId };
    case 'hmr:module-registered':
      return { type: 'hmr:module-registered', modules: decoded.modules };
    default:
      const _never: never = decoded;
      throw new Error(`Unknown client message: ${stringified}`);
  }
}
