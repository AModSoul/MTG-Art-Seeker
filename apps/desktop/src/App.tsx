import { Box, Center, Flex, Grid, Spinner } from "@chakra-ui/react";
import { useEffect } from "react";

import { AppHeader } from "@/components/AppHeader";
import { FoundArtDialog } from "@/components/FoundArtDialog";
import { InputPanel } from "@/components/InputPanel";
import { PageImagesDialog } from "@/components/PageImagesDialog";
import { RunView } from "@/components/RunView";
import { ScrollArea } from "@mtg-art/ui/scroll-area";
import { useSettings } from "@/store";

export function App() {
  const loaded = useSettings((state) => state.settings !== null);
  const load = useSettings((state) => state.load);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <Flex direction="column" h="100vh" bg="bg">
      <AppHeader />
      {loaded ? (
        <Grid templateColumns="minmax(320px, 400px) 1fr" flex="1" minH="0">
          {/* Full height, so the panel's own lists scroll before it does. */}
          <ScrollArea borderEndWidth="1px" minH="0" contentProps={{ h: "full" }}>
            <InputPanel />
          </ScrollArea>
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
