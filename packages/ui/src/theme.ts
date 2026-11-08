import { createSystem, defaultConfig, defineConfig } from "@chakra-ui/react";

import { button } from "./recipes/button.recipe";
import { progress } from "./recipes/progress.recipe";
import { radioCard } from "./recipes/radio-card.recipe";
import { radioGroup } from "./recipes/radio-group.recipe";
import { segmentGroup } from "./recipes/segment-group.recipe";
import { switchRecipe } from "./recipes/switch.recipe";
import { tabs } from "./recipes/tabs.recipe";
import { toast } from "./recipes/toast.recipe";

/** Chakra's own fallbacks, for the moment before Inter loads. */
const FALLBACK_FONTS = `-apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol"`;

/**
 * The app's Chakra system. Tokens and semantic tokens go in this config; recipes and slot recipes
 * live in `./recipes`, one file each, and only add to Chakra's own. Run
 * `pnpm --filter @mtg-art/ui typegen` after changing any of them so their props are typed.
 */
const config = defineConfig({
  theme: {
    tokens: {
      // Chakra's stack names Inter but doesn't ship it, so it only showed where it was installed.
      // `./fonts.css`, which the provider and the Storybook preview import, declares it as
      // "Inter Variable".
      fonts: {
        heading: { value: `"Inter Variable", ${FALLBACK_FONTS}` },
        body: { value: `"Inter Variable", ${FALLBACK_FONTS}` },
      },
      colors: {
        // The default accent (see `./recipes`): emerald, #50C878, at 500.
        emerald: {
          50: { value: "#EDFAF2" },
          100: { value: "#D3F3E0" },
          200: { value: "#A9E7C3" },
          300: { value: "#7FDBA6" },
          400: { value: "#66D18F" },
          500: { value: "#50C878" },
          600: { value: "#3BA862" },
          700: { value: "#2E844D" },
          800: { value: "#23633A" },
          900: { value: "#184428" },
          950: { value: "#0D2817" },
        },
      },
    },
    semanticTokens: {
      colors: {
        // Chakra's 500 and 600 are under 4.5:1 on white for the small error and warning
        // lines under a card; 700 reads. Dark mode keeps Chakra's.
        fg: {
          error: { value: { _light: "{colors.red.700}", _dark: "{colors.red.400}" } },
          warning: { value: { _light: "{colors.orange.700}", _dark: "{colors.orange.300}" } },
        },
        // What `colorPalette="emerald"` reads. The fill is #50C878 in both modes; its text is
        // a dark green, since white on it is too faint to read. Light mode's fg is 800: 700
        // reads on white but not on `subtle` (3.9:1), as a subtle button's text is.
        emerald: {
          solid: { value: "{colors.emerald.500}" },
          contrast: { value: "{colors.emerald.950}" },
          fg: { value: { _light: "{colors.emerald.800}", _dark: "{colors.emerald.300}" } },
          muted: { value: { _light: "{colors.emerald.200}", _dark: "{colors.emerald.800}" } },
          subtle: { value: { _light: "{colors.emerald.100}", _dark: "{colors.emerald.900}" } },
          emphasized: { value: { _light: "{colors.emerald.300}", _dark: "{colors.emerald.700}" } },
          focusRing: { value: "{colors.emerald.500}" },
          border: { value: { _light: "{colors.emerald.500}", _dark: "{colors.emerald.400}" } },
        },
      },
    },
    recipes: { button },
    slotRecipes: {
      toast,
      switch: switchRecipe,
      radioGroup,
      radioCard,
      segmentGroup,
      progress,
      tabs,
    },
  },
});

export const system = createSystem(defaultConfig, config);
