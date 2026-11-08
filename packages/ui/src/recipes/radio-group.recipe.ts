import { defineSlotRecipe } from "@chakra-ui/react";
import { radioGroupAnatomy } from "@chakra-ui/react/anatomy";

/** Radios are emerald unless they name a palette. */
export const radioGroup = defineSlotRecipe({
  slots: radioGroupAnatomy.keys(),
  base: {
    root: { colorPalette: "emerald" },
  },
});
