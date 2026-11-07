import { createSystem, defaultConfig, defineConfig } from "@chakra-ui/react";

/**
 * The app's Chakra system. Tokens, semantic tokens, recipes and slot recipes go in this config;
 * run `npm run typegen` after changing it so their props are typed.
 */
const config = defineConfig({});

export const system = createSystem(defaultConfig, config);
