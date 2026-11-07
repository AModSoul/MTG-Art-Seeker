import { Box, Image } from "@chakra-ui/react";

// One copy of each screenshot, shared with the README.
import resultsDark from "../../../../docs/screenshots/results-dark.png";
import resultsLight from "../../../../docs/screenshots/results-light.png";

/** The app after a run, in whichever colour mode the page is in. */
export function Screenshot() {
  const alt =
    "The app after downloading a deck list: each card with a thumbnail of its art, where the art came from, and the artist";
  return (
    <Box rounded="xl" overflow="hidden" borderWidth="1px" shadow="lg">
      <Image src={resultsLight} alt={alt} w="full" _dark={{ display: "none" }} />
      <Image src={resultsDark} alt={alt} w="full" display="none" _dark={{ display: "block" }} />
    </Box>
  );
}
