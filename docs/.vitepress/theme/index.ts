import DefaultTheme from 'vitepress/theme';
import './custom.css';
import LayoutWithRedirect from './LayoutWithRedirect.vue';
import 'virtual:group-icons.css';

export default {
  ...DefaultTheme,
  Layout: LayoutWithRedirect,
};
