import { defineSlotRecipe } from "@chakra-ui/react";
import { switchAnatomy } from "@chakra-ui/react/anatomy";

/** Switches are emerald unless they name a palette. */
export const switchRecipe = defineSlotRecipe({
  slots: switchAnatomy.keys(),
  base: {
    root: { colorPalette: "emerald" },
  },
});
