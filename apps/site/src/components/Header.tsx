import { Box, Container, HStack, IconButton, Image, Link } from "@chakra-ui/react";
import { ColorModeButton } from "@mtg-art/ui/color-mode";
import { Link as RouterLink } from "@tanstack/react-router";
import { LuGithub } from "react-icons/lu";

import { APP_NAME, GITHUB_REPO_URL } from "@/site-config";

const NAV = [
  { to: "/", label: "Home" },
  { to: "/download", label: "Download" },
] as const;

export function Header() {
  return (
    <Box
      as="header"
      position="sticky"
      top={0}
      zIndex="sticky"
      borderBottomWidth="1px"
      bg="bg/80"
      backdropFilter="blur(12px)"
    >
      <Container maxW="6xl" py={3}>
        <HStack justify="space-between" gap={4}>
          <Link asChild fontWeight="semibold" color="fg" textDecoration="none" gap={2}>
            <RouterLink to="/">
              <Image src={`${import.meta.env.BASE_URL}favicon.svg`} alt="" boxSize={7} />
              <Box as="span" display={{ base: "none", sm: "inline" }}>
                {APP_NAME}
              </Box>
            </RouterLink>
          </Link>

          <HStack as="nav" aria-label="Main" gap={{ base: 3, sm: 5 }} fontSize="sm">
            {NAV.map(({ to, label }) => (
              <Link key={to} asChild color="fg.muted" _hover={{ color: "fg" }}>
                <RouterLink
                  to={to}
                  activeOptions={{ exact: true }}
                  activeProps={{ "aria-current": "page", style: { fontWeight: 600 } }}
                >
                  {label}
                </RouterLink>
              </Link>
            ))}
            <HStack gap={1}>
              <IconButton asChild variant="ghost" size="sm" aria-label="Source code on GitHub">
                <a href={GITHUB_REPO_URL} target="_blank" rel="noreferrer">
                  <LuGithub />
                </a>
              </IconButton>
              <ColorModeButton />
            </HStack>
          </HStack>
        </HStack>
      </Container>
    </Box>
  );
}
