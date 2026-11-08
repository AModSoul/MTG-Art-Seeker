import { defineSlotRecipe } from "@chakra-ui/react";
import { progressAnatomy } from "@chakra-ui/react/anatomy";

/** Progress bars are emerald unless they name a palette. */
export const progress = defineSlotRecipe({
  slots: progressAnatomy.keys(),
  base: {
    root: { colorPalette: "emerald" },
  },
});
