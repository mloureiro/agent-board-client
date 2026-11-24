import postcss from 'postcss';
import presetMantine from 'postcss-preset-mantine';
import simpleVars from 'postcss-simple-vars';

const nativeApp = postcss([
  presetMantine(),
  simpleVars({
    variables: {
      'mantine-breakpoint-xs': '36em',
      'mantine-breakpoint-sm': '48em',
      'mantine-breakpoint-md': '62em',
      'mantine-breakpoint-lg': '75em',
      'mantine-breakpoint-xl': '88em',
    },
  }),
]);

// The Mantine preset rewrites light-dark() into [data-mantine-color-scheme] selectors, which only
// the native app sets. Running it over the server list or tab bar would pin them to light.
/** @type {import("postcss").Plugin} */
const onlyInNativeApp = {
  postcssPlugin: 'only-in-native-app',
  async Once(root, { result }) {
    const file = result.opts.from ?? '';
    if (file.replaceAll('\\', '/').includes('/src/board/')) {
      await nativeApp.process(root, { from: result.opts.from });
    }
  },
};

export default { plugins: [onlyInNativeApp] };
