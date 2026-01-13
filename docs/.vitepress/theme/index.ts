import RolldownTheme from '@voidzero-dev/vitepress-theme/src/rolldown';
import 'virtual:group-icons.css';
import type { Theme } from 'vitepress';
import DefinedIn from './components/DefinedIn.vue';
import './styles.css';

export default {
  extends: RolldownTheme,
  enhanceApp({ app }) {
    app.component('DefinedIn', DefinedIn);
  },
} satisfies Theme;
