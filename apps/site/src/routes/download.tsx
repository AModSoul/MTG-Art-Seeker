import { Button, Container, Heading, Stack, Text } from "@chakra-ui/react";
import { createFileRoute } from "@tanstack/react-router";

import { APP_NAME, GITHUB_RELEASES_URL } from "@/site-config";

export const Route = createFileRoute("/download")({
  component: Download,
  head: () => ({ meta: [{ title: `Download · ${APP_NAME}` }] }),
});

function Download() {
  return (
    <Container maxW="4xl" py={{ base: 10, md: 16 }}>
      <Stack gap={4} align="flex-start">
        <Heading as="h1" size="3xl">
          Download
        </Heading>
        <Text color="fg.muted">Installers for Windows, macOS and Linux are on GitHub.</Text>
        <Button asChild colorPalette="blue" size="lg">
          <a href={`${GITHUB_RELEASES_URL}/latest`}>Latest release</a>
        </Button>
      </Stack>
    </Container>
  );
}
