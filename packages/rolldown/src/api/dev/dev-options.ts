import type { BindingHmrUpdate } from '../../binding';

export interface DevOptions {
  onHmrUpdates?: (updates: BindingHmrUpdate[]) => void | Promise<void>;
}
