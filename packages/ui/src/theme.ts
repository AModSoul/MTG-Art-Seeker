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
  },
});

export const system = createSystem(defaultConfig, config);
