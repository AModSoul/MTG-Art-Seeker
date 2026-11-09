// The preview setup every Storybook in the workspace shares: the theme switcher,
// the Chakra system, a toaster, and the a11y addon's settings. A package's
// `.storybook/preview.tsx` spreads `basePreview`, or `previewFor` its own system,
// and calls `resetUi` from its own `beforeEach`.

import "./fonts.css";
import { ChakraProvider, type SystemContext } from "@chakra-ui/react";
import { withThemeByClassName } from "@storybook/addon-themes";
import type { Preview } from "@storybook/react-vite";

import { Toaster, toaster } from "./components/ui/toaster";
import { system as sharedSystem } from "./theme";

/** The shared preview over `system`: an app's own, built on the shared theme (`theme.ts`). */
export const previewFor = (system: SystemContext) =>
  ({
    decorators: [
      withThemeByClassName({
        themes: { light: "", dark: "dark" },
        defaultTheme: "light",
      }),
      // ChakraProvider rather than the app's Provider: next-themes would fight
      // the themes addon over the class on <html>.
      (Story) => (
        <ChakraProvider value={system}>
          <Story />
          <Toaster />
        </ChakraProvider>
      ),
    ],
    parameters: {
      controls: {
        matchers: {
          color: /(background|color)$/i,
          date: /Date$/i,
        },
      },
      a11y: {
        // 'todo' - show a11y violations in the test UI only
        // 'error' - fail CI on a11y violations
        // 'off' - skip a11y checks entirely
        test: "todo",
      },
    },
  }) satisfies Preview;

/** The shared preview over the shared system. */
export const basePreview = previewFor(sharedSystem);

/** Clears what one story leaves behind for the next, such as toasts. */
export function resetUi() {
  toaster.remove();
}
