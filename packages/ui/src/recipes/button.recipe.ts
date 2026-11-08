import { defineRecipe } from "@chakra-ui/react";

/**
 * Filled buttons are emerald unless they name a palette; outline, ghost and plain ones stay gray,
 * so secondary actions and icon buttons don't turn green.
 */
export const button = defineRecipe({
  variants: {
    variant: {
      solid: { colorPalette: "emerald" },
      subtle: { colorPalette: "emerald" },
      surface: { colorPalette: "emerald" },
    },
  },
});
