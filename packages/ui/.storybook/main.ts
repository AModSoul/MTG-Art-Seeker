import type { StorybookConfig } from "@storybook/react-vite";

// The shared component library. The app Storybooks show these stories too, by
// composing this one as a ref.
const config: StorybookConfig = {
  stories: ["../src/**/*.stories.@(ts|tsx)"],
  addons: [
    "@storybook/addon-vitest",
    "@storybook/addon-a11y",
    "@storybook/addon-docs",
    "@storybook/addon-themes",
    "@storybook/addon-mcp",
  ],
  framework: "@storybook/react-vite",
  features: {
    // Required by the MCP addon's docs toolset (docs-list, docs-show).
    componentsManifest: true,
  },
};
export default config;
