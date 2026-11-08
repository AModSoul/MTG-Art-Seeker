import { defineSlotRecipe } from "@chakra-ui/react";
import { radioCardAnatomy } from "@chakra-ui/react/anatomy";

/** Radio cards are emerald unless they name a palette. */
export const radioCard = defineSlotRecipe({
  slots: radioCardAnatomy.keys(),
  base: {
    root: { colorPalette: "emerald" },
  },
});
