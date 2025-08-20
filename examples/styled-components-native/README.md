# Styled Components Native Example

This example demonstrates using styled-components with Rolldown bundler, showcasing native CSS-in-JS styling capabilities.

## Features

- **React Components**: Modern React with hooks and functional components
- **Styled Components**: CSS-in-JS styling with template literals
- **Dynamic Styling**: Props-based conditional styling and theming
- **Advanced CSS**: Gradients, transitions, animations, and hover effects
- **Component Architecture**: Reusable styled components with clean separation

## Components Demonstrated

### Button Component

- Multiple variants (primary, secondary, default)
- Hover effects with smooth transitions
- Disabled state handling
- CSS-in-JS conditional styling with `css` helper
- Ripple effect animation

### Card Component

- Glass morphism design with backdrop blur
- Hover animations with transform effects
- Flexible content layout
- Shadow and border styling

### App Layout

- CSS Grid responsive layout
- Gradient backgrounds
- Typography styling with web fonts

## Key Styled Components Features

1. **Template Literal Syntax**: Native CSS syntax within JavaScript
2. **Props Integration**: Dynamic styling based on component props
3. **CSS Helper**: Conditional styling blocks with the `css` function
4. **Pseudo-selectors**: `:hover`, `:focus`, `:disabled`, `::before` support
5. **Animations**: Smooth transitions and keyframe animations
6. **Theme Support**: Consistent design system across components

## OXC Styled Components Transform

This example uses Rolldown's built-in OXC transform for styled-components, which provides:

- **Display Names**: Enhanced debugging with component names in DevTools
- **File Names**: Component names prefixed with filename for uniqueness
- **SSR Support**: Unique component IDs to avoid hydration mismatches
- **Template Literal Transpilation**: Smaller bundle size with optimized output
- **CSS Minification**: Removes whitespace and comments from CSS
- **Pure Annotations**: Enables dead code elimination by bundlers
- **Namespace Support**: Prefixes component IDs for style isolation

The transform is configured in `rolldown.config.js`:

```javascript
export default {
  // ... snip
  transform: {
    plugins: {
      styledComponents: {
        displayName: true,
        fileName: true,
        ssr: true,
        transpileTemplateLiterals: true,
        minify: true,
        pure: true,
        namespace: 'rolldown-example',
      },
    },
  },
};
```

## Running the Example

```bash
# Install dependencies
pnpm install

# Build the project
pnpm build
```

Open `index.html` in your browser to see the styled components in action.

## File Structure

```
src/
  ├── index.jsx          # React app entry point
  ├── App.jsx            # Main app component with layout
  └── components/
      ├── Button.jsx     # Styled button component
      └── Card.jsx       # Styled card component
```

This example showcases how Rolldown efficiently bundles styled-components while preserving all CSS-in-JS functionality and runtime styling capabilities.
