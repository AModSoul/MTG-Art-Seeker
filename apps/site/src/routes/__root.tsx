import { Box, Flex } from "@chakra-ui/react";
import { Provider } from "@mtg-art/ui/provider";
import { createRootRoute, Outlet, useMatches } from "@tanstack/react-router";
import { useLayoutEffect } from "react";

import { Footer } from "@/components/Footer";
import { Header } from "@/components/Header";
import { documentTitleFromMatches } from "@/document-title";
import { APP_NAME, DESCRIPTION, SITE_URL } from "@/site-config";

const IMAGE = `${SITE_URL}icon-512.png`;

export const Route = createRootRoute({
  component: RootLayout,
  head: () => ({
    meta: [
      { title: APP_NAME },
      { name: "description", content: DESCRIPTION },
      { property: "og:type", content: "website" },
      { property: "og:site_name", content: APP_NAME },
      { property: "og:title", content: APP_NAME },
      { property: "og:description", content: DESCRIPTION },
      { property: "og:url", content: SITE_URL },
      { property: "og:image", content: IMAGE },
      { name: "twitter:card", content: "summary" },
    ],
  }),
});

function RootLayout() {
  const matches = useMatches();

  useLayoutEffect(() => {
    const title = documentTitleFromMatches(matches);
    if (title) document.title = title;
  }, [matches]);

  return (
    <Provider>
      <Flex direction="column" minH="100dvh">
        <Header />
        <Box as="main" flex={1}>
          <Outlet />
        </Box>
        <Footer />
      </Flex>
    </Provider>
  );
}
