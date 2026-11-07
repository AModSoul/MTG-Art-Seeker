import { Container, Heading, Stack, Text } from "@chakra-ui/react";
import { createFileRoute } from "@tanstack/react-router";

import { DownloadPanel } from "@/components/DownloadPanel";
import { useLatestRelease, useVisitorPlatform } from "@/downloads/hooks";
import { APP_NAME } from "@/site-config";

export const Route = createFileRoute("/download")({
  component: Download,
  head: () => ({ meta: [{ title: `Download · ${APP_NAME}` }] }),
});

function Download() {
  const state = useLatestRelease();
  const { os, arch } = useVisitorPlatform();

  return (
    <Container maxW="4xl" py={{ base: 10, md: 16 }}>
      <Stack gap={8}>
        <Stack gap={2}>
          <Heading as="h1" size="3xl">
            Download
          </Heading>
          <Text color="fg.muted" fontSize="lg">
            The desktop app for Windows, macOS and Linux. It checks for new versions when it starts
            and offers to update itself.
          </Text>
        </Stack>
        <DownloadPanel state={state} os={os} arch={arch} />
      </Stack>
    </Container>
  );
}
