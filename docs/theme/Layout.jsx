import Doc from './layouts/Doc.jsx';
import Home from './layouts/Home.jsx';
import NotFound from './layouts/NotFound.jsx';

// The layouts pages select with `layout` in their frontmatter
const LAYOUTS = { home: Home };

/**
 * Every page is wrapped in this layout: the 404 page's, the one its
 * frontmatter selects, or the documentation layout.
 *
 * @param {{ metadata: object, headings: Array, children: import('preact').ComponentChildren }} props
 */
export default (props) => {
  const Component =
    props.metadata.api === '404' ? NotFound : (LAYOUTS[props.metadata.layout] ?? Doc);

  return <Component {...props} />;
};
