import type { HTMLChakraProps, RecipeProps } from "@chakra-ui/react";
import { createRecipeContext } from "@chakra-ui/react";

export type LinkButtonProps = HTMLChakraProps<"a", RecipeProps<"button">>;

const { withContext } = createRecipeContext({ key: "button" });

/** A link that looks like a button. Give it `href`, or `asChild` with a router's link. */
export const LinkButton = withContext<HTMLAnchorElement, LinkButtonProps>("a");
