import { defineSlotRecipe } from "@chakra-ui/react";
import { tabsAnatomy } from "@chakra-ui/react/anatomy";

/**
 * Tabs are emerald unless they name a palette. It's set on the list, not the root, so what's
 * inside a tab's content doesn't inherit it.
 */
export const tabs = defineSlotRecipe({
  slots: tabsAnatomy.keys(),
  base: {
    list: { colorPalette: "emerald" },
  },
});
