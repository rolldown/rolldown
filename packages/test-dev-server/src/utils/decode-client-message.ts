import { ClientMessage } from '../types/client-message';

export function decodeClientMessage(message: string): ClientMessage {
  const data = JSON.parse(message);
  if (data.type === 'hmr:invalidate') {
    return { type: 'hmr:invalidate', moduleId: data.moduleId };
  }

  throw new Error(`Unknown client message: ${message}`);
}
