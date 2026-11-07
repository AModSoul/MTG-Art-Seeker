import { Box, Container, HStack, Link, Stack, Text } from "@chakra-ui/react";

import { DISCORD_INVITE_URL, GITHUB_RELEASES_URL, GITHUB_REPO_URL } from "@/site-config";

const LINKS = [
  { href: GITHUB_REPO_URL, label: "GitHub" },
  { href: GITHUB_RELEASES_URL, label: "Releases" },
  { href: DISCORD_INVITE_URL, label: "Discord" },
];

export function Footer() {
  return (
    <Box as="footer" mt={16} borderTopWidth="1px" py={8}>
      <Container maxW="6xl">
        <Stack gap={3} align="center" textAlign="center">
          <HStack gap={6} wrap="wrap" justify="center">
            {LINKS.map(({ href, label }) => (
              <Link key={href} href={href} target="_blank" rel="noreferrer" fontSize="sm">
                {label}
              </Link>
            ))}
          </HStack>
          <Text fontSize="xs" color="fg.muted">
            Free software under the GPL-3.0. Not affiliated with Wizards of the Coast, Scryfall or
            MTGPics.
          </Text>
        </Stack>
      </Container>
    </Box>
  );
}
