import messages from './translations.js';
import { transferForm } from './form.js';

export default function Page() {
  return [messages.title, transferForm().name];
}
