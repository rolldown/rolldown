import RolldownTheme from '@voidzero-dev/vitepress-theme/src/rolldown';
import 'virtual:group-icons.css';
import 'vitepress-plugin-graphviz/style.css';
import { feedbackPlugin } from 'vitepress-plugin-feedback-tracker';
import 'vitepress-plugin-feedback-tracker/style.css';
import type { Theme } from 'vitepress';
import DefinedIn from './components/DefinedIn.vue';
import Layout from './Layout.vue';
import './styles.css';

export default {
  extends: RolldownTheme,
  Layout,
  enhanceApp(ctx) {
    ctx.app.component('DefinedIn', DefinedIn);
    const feedback = feedbackPlugin({
      siteId: 'rolldown',
      apiUrl: 'https://vitepress-feedback-api.void.app',
    });
    feedback.enhanceApp?.(ctx);
  },
} satisfies Theme;
