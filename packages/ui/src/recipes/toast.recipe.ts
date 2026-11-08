import { defineSlotRecipe } from "@chakra-ui/react";
import { toastAnatomy } from "@chakra-ui/react/anatomy";

/** Adds to Chakra's own toast. */
export const toast = defineSlotRecipe({
  slots: toastAnatomy.keys(),
  base: {
    root: {
      // An error toast's text is faded to 80%, which on Chakra's red.600 is under
      // 4.5:1; on red.700 it reads. Dark mode keeps Chakra's.
      "&[data-type=error]": { bg: { _light: "red.700", _dark: "red.solid" } },
    },
  },
});
