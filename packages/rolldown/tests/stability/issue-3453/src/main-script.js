import * as React from 'react';
import * as ReactDOM from 'react-dom/client';
import * as Framer from 'framer';

const routes = {
  DeL0q0H0v: {
    elements: {},
    page: Framer.lazy(() =>
      import(
        'https://framerusercontent.dev/modules/cUmP60oZsBS7gKsHEyLv/4z3ICMttLBn1hxcyCoio/DeL0q0H0v.js'
      )
    ),
    path: '/',
  },
  xOoeiNxXJ: {
    elements: {},
    page: Framer.lazy(() =>
      import(
        'https://framerusercontent.dev/modules/E754SxKhDhOsCwkeFkVd/zaN2rwE0nygiUpmXLsYa/xOoeiNxXJ.js'
      )
    ),
    path: '/blog',
  },
  v2piBMke6: {
    elements: { bBkTmB1xq: 'hello' },
    page: Framer.lazy(() =>
      import(
        'https://framerusercontent.dev/modules/sJVl05lpwHddjmPmrB3B/lNmQ2oXzskCps1syixEY/v2piBMke6.js'
      )
    ),
    path: '/page',
  },
  Gkwjsv2el: {
    collectionId: 'U5J_P2oWm',
    elements: {},
    page: Framer.lazy(() =>
      import(
        'https://framerusercontent.dev/modules/zLFB8zCF8UV1x0ywwZvx/e3aGBuhO3d2mJhmbfnvK/Gkwjsv2el.js'
      )
    ),
    path: '/blog/:l1S5PdTV6',
  },
};
const locales = [
  { code: 'en-US', id: 'default', name: 'English', slug: '' },
  { code: 'pl-PL', id: 'PLJA3RgJE', name: 'Polish (Poland)', slug: 'pl' },
];

export async function getPageRoot({ routeId, pathVariables, localeId }) {
  // We don't want the initial render to immediately have to suspend.
  await routes[routeId].page.preload();

  const content = React.createElement(Framer.PageRoot, {
    isWebsite: true,
    routeId,
    pathVariables,
    routes,
    collectionUtils: {
      U5J_P2oWm: async () =>
        (
          await import(
            'https://framerusercontent.dev/modules/yclpWreWbUYg71AtgF0p/U88ywOxGdYeFqXAjpfMC/U5J_P2oWm.js'
          )
        )?.['utils'],
    },
    framerSiteId:
      '8db1fbbd12a00a7823a7a7b0d30082d7a2b3e7608b19921c4e198f5e3af760e8',
    notFoundPage: Framer.lazy(() => import('__framer-not-found-page')),
    isReducedMotion: undefined,
    localeId,
    locales,
    preserveQueryParams: true,
  });

  const contentWithFeaturesContext = React.createElement(
    Framer.LibraryFeaturesProvider,
    {
      children: content,
      value: {
        codeBoundaries: true,
        editorBarMenu: true,
        enableAsyncURLUpdates: true,
        replaceNestedLinks: true,
        useGranularSuspense: true,
        wrapUpdatesInTransitions: true,
      },
    }
  );

  const contentWithGracefullyDegradingErrorBoundary = React.createElement(
    Framer.GracefullyDegradingErrorBoundary,
    {
      children: contentWithFeaturesContext,
    }
  );

  const effect = {
    enter: {
      opacity: 0,
      rotate: 0,
      rotate3d: false,
      rotateX: 0,
      rotateY: 0,
      scale: 1,
      transition: {
        damping: 30,
        delay: 0,
        duration: 1,
        ease: [0.27, 0, 0.51, 1],
        mass: 1,
        stiffness: 400,
        type: 'tween',
      },
      x: '0px',
      y: '0px',
    },
  };
  const page = React.createElement(Framer.PageEffectsProvider, {
    children: contentWithGracefullyDegradingErrorBoundary,
    value: {
      global: {
        enter: {
          opacity: 1,
          rotate: 0,
          rotate3d: false,
          rotateX: 0,
          rotateY: 0,
          scale: 1,
          transition: {
            damping: 30,
            delay: 0,
            duration: 0.2,
            ease: [0.27, 0, 0.51, 1],
            mass: 1,
            stiffness: 400,
            type: 'tween',
          },
          x: '100px',
          y: '0px',
        },
      },
      routes: { xOoeiNxXJ: { Gkwjsv2el: effect } },
    },
  });

  return page;
}

const isBrowser = typeof document !== 'undefined';
if (isBrowser) {
  window.__framer_importFromPackage =
    (packageAndFilename, exportIdentifier) => () => {
      return React.createElement(Framer.ErrorPlaceholder, {
        error:
          'Package component not supported: "' +
          exportIdentifier +
          '" in "' +
          packageAndFilename +
          '"',
      });
    };

  // A lot of libraries assume process.env.NODE_ENV is present in runtime/buildtime, so we are polyfilling it
  window.process = {
    ...window.process,
    env: {
      ...(window.process ? window.process.env : undefined),
      NODE_ENV: 'production',
    },
  };

  window.__framer_events = window.__framer_events || [];

  // Fallback support for stack gaps
  Framer.installFlexboxGapWorkaroundIfNeeded();

  const container = document.getElementById('main');
  // We know that #main is parsed before this script, so we don't need to wait for DOMContentLoaded or similar events.
  if ('framerHydrateV2' in container.dataset) main(true, container);
  else main(false, container);
}

function track() {
  if (!isBrowser) return;
  window.__framer_events.push(arguments);
}

async function main(shouldHydrate, container) {
  function handleError(error, errorInfo, recoverable = true) {
    if (error.caught || window.__framer_hadFatalError) return; // we already logged it

    const componentStack = errorInfo?.componentStack;
    if (recoverable) {
      console.warn(
        'Recoverable error during hydration. Please check any custom code or code overrides to fix server/client mismatches:\n',
        error,
        componentStack
      );
      // we only want to collect 1%, because this can be quite noisy (floods the data pipeline)
      if (Math.random() > 0.01) return;
    } else {
      console.error(
        'Fatal crash during hydration. If you are the author of this website, please report this issue to the Framer team via https://www.framer.community/'
      );
    }
    track(
      recoverable
        ? 'published_site_load_recoverable_error'
        : 'published_site_load_error',
      {
        message: String(error),
        componentStack, // componentStack is more useful
        stack: componentStack
          ? undefined
          : error instanceof Error && typeof error.stack === 'string'
          ? error.stack
          : null,
      }
    );
  }

  try {
    let routeId, localeId, pathVariables, breakpoints;
    if (shouldHydrate) {
      const routeData = JSON.parse(container.dataset.framerHydrateV2);
      routeId = routeData.routeId;
      localeId = routeData.localeId;
      pathVariables = routeData.pathVariables;
      breakpoints = routeData.breakpoints;
    } else {
      const routeData = Framer.inferInitialRouteFromPath(
        routes,
        decodeURIComponent(location.pathname),
        true,
        locales
      );
      routeId = routeData.routeId;
      localeId = routeData.localeId;
      pathVariables = routeData.pathVariables;
    }

    const page = await getPageRoot({ routeId, localeId, pathVariables });
    if (shouldHydrate) {
      Framer.withPerformanceMarks('framer-rewrite-breakpoints', () => {
        Framer.removeHiddenBreakpointLayersV2(breakpoints);
        window.__framer_onRewriteBreakpoints?.(breakpoints);
      });

      const startTransition = React.startTransition;
      startTransition(() => {
        Framer.markHydrationStart();
        Framer.setInitialHydrationState();
        if (true) Framer.turnOffReactEventHandling();
        ReactDOM.hydrateRoot(container, page, {
          onRecoverableError: handleError,
        });
      });
    } else {
      ReactDOM.createRoot(container, {
        onRecoverableError: handleError,
      }).render(page);
    }
  } catch (error) {
    handleError(error, undefined, false);
    throw error;
  }
}
