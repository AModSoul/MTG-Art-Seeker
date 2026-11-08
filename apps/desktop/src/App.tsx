import { Box, Center, Flex, Grid, Heading, HStack, Spinner } from "@chakra-ui/react";
import { useEffect } from "react";

import { DiscordButton } from "@/components/DiscordButton";
import { FoundArtDialog } from "@/components/FoundArtDialog";
import { InputPanel } from "@/components/InputPanel";
import { PageImagesDialog } from "@/components/PageImagesDialog";
import { RunView } from "@/components/RunView";
import { SettingsDrawer } from "@/components/SettingsDrawer";
import { ColorModeButton } from "@mtg-art/ui/color-mode";
import { UpdateButton } from "@/components/UpdateButton";
import { useSettings } from "@/store";

export function App() {
  const loaded = useSettings((state) => state.settings !== null);
  const load = useSettings((state) => state.load);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <Flex direction="column" h="100vh" bg="bg">
      <HStack px="5" py="3" borderBottomWidth="1px" gap="3">
        <Heading size="md" flex="1">
          MTG Art Downloader
        </Heading>
        <UpdateButton />
        <DiscordButton />
        <ColorModeButton />
        <SettingsDrawer />
      </HStack>
      {loaded ? (
        <Grid templateColumns="minmax(320px, 400px) 1fr" flex="1" minH="0">
          <Box borderEndWidth="1px" minH="0" overflowY="auto">
            <InputPanel />
          </Box>
          <Box minH="0" minW="0">
            <RunView />
          </Box>
        </Grid>
      ) : (
        <Center flex="1">
          <Spinner />
        </Center>
      )}
      <FoundArtDialog />
      <PageImagesDialog />
    </Flex>
  );
}
