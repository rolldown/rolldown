import DefaultTheme from 'vitepress/theme';
import './custom.css';
import 'virtual:group-icons.css';
import type { Theme } from 'vitepress';
import DefinedIn from './components/DefinedIn.vue';

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('DefinedIn', DefinedIn);
  },
} satisfies Theme;
