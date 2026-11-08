import { defineSlotRecipe } from "@chakra-ui/react";
import { segmentGroupAnatomy } from "@chakra-ui/react/anatomy";

/** Segmented controls are emerald unless they name a palette. */
export const segmentGroup = defineSlotRecipe({
  slots: segmentGroupAnatomy.keys(),
  base: {
    root: { colorPalette: "emerald" },
  },
});
