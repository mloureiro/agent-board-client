declare module 'postcss-preset-mantine' {
  import type { PluginCreator } from 'postcss';

  const presetMantine: PluginCreator<Record<string, unknown>>;
  export default presetMantine;
}
