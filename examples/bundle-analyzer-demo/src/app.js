import { Header } from './components/header.js';
import { Footer } from './components/footer.js';
import { Router } from './router.js';

export function createApp() {
  return { header: Header, footer: Footer, router: Router };
}
