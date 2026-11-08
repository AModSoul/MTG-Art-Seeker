import { createSystem, defaultConfig, defineConfig } from "@chakra-ui/react";
/**
 * The app's Chakra system. Tokens, semantic tokens, recipes and slot recipes go in this config;
 * run `npm run typegen` after changing it so their props are typed.
 */
const config = defineConfig({
  theme: {
    tokens: {
      colors: {
        // The download button and progress bar: emerald, #50C878, at 500.
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
        // a dark green, since white on it is too faint to read.
        emerald: {
          solid: { value: "{colors.emerald.500}" },
          contrast: { value: "{colors.emerald.950}" },
          fg: { value: { _light: "{colors.emerald.700}", _dark: "{colors.emerald.300}" } },
          muted: { value: { _light: "{colors.emerald.200}", _dark: "{colors.emerald.800}" } },
          subtle: { value: { _light: "{colors.emerald.100}", _dark: "{colors.emerald.900}" } },
          emphasized: { value: { _light: "{colors.emerald.300}", _dark: "{colors.emerald.700}" } },
          focusRing: { value: "{colors.emerald.500}" },
          border: { value: { _light: "{colors.emerald.500}", _dark: "{colors.emerald.400}" } },
        },
      },
    },
    slotRecipes: {
      toast: {
        // Chakra's own toast's parts, which this only adds to.
        slots: defaultConfig.theme?.slotRecipes?.toast?.slots ?? [],
        base: {
          root: {
            // An error toast's text is faded to 80%, which on Chakra's red.600 is under
            // 4.5:1; on red.700 it reads. Dark mode keeps Chakra's.
            "&[data-type=error]": { bg: { _light: "red.700", _dark: "red.solid" } },
          },
        },
      },
    },
  },
});

export const system = createSystem(defaultConfig, config);
