import { createSystem, defaultConfig, defineConfig, defineSlotRecipe } from "@chakra-ui/react";
import { dialogAnatomy, drawerAnatomy } from "@chakra-ui/react/anatomy";

import { HEADER_HEIGHT } from "@/layout";
import { config } from "@mtg-art/ui/theme";

/** Where an overlay's backdrop and positioner sit: below the header, which is the window's
 * title bar and is never covered. */
const belowHeader = { top: HEADER_HEIGHT, h: `calc(100dvh - ${HEADER_HEIGHT})` };

/** The desktop app's own layer over the theme it shares with the site: every dialog and drawer
 * opens below the header. The header itself stays usable while one is open through
 * `keepHeader` (`layout.ts`). Like the shared recipes, these only add to Chakra's own. */
const desktop = defineConfig({
  theme: {
    slotRecipes: {
      dialog: defineSlotRecipe({
        slots: dialogAnatomy.keys(),
        base: { backdrop: belowHeader, positioner: belowHeader },
      }),
      drawer: defineSlotRecipe({
        slots: drawerAnatomy.keys(),
        base: { backdrop: belowHeader, positioner: belowHeader },
      }),
    },
  },
});

export const system = createSystem(defaultConfig, config, desktop);
