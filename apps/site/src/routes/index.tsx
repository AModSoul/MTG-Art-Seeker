import {
  Box,
  Code,
  Container,
  Heading,
  HStack,
  Icon,
  Link,
  SimpleGrid,
  Stack,
  Text,
} from "@chakra-ui/react";
import { Card } from "@mtg-art/ui/card";
import { LinkButton } from "@mtg-art/ui/link-button";
import { ScrollArea } from "@mtg-art/ui/scroll-area";
import { createFileRoute, Link as RouterLink } from "@tanstack/react-router";
import type { IconType } from "react-icons";
import {
  LuDownload,
  LuFileText,
  LuFolderTree,
  LuImage,
  LuPackage,
  LuScanSearch,
  LuTerminal,
  LuZap,
} from "react-icons/lu";

import { Screenshot } from "@/components/Screenshot";
import { GITHUB_REPO_URL } from "@/site-config";

export const Route = createFileRoute("/")({
  component: Home,
});

const FEATURES: { icon: IconType; title: string; body: string }[] = [
  {
    icon: LuImage,
    title: "Larger, uncropped art",
    body: "Art comes from MTGPics where it has it, and from Scryfall otherwise. A page counts as a match only when it names the card and credits its artist, so you never get the wrong art.",
  },
  {
    icon: LuScanSearch,
    title: "Find a larger copy",
    body: "When Scryfall's small art is all there is, look for a bigger copy with Google Lens or on the artist's ArtStation, DeviantArt, Pixiv or website. The app checks what you save is the same art, larger, and every later download of it gets your copy.",
  },
  {
    icon: LuFileText,
    title: "Deck lists and searches",
    body: "Paste or open the lists deck sites export: a line with a set and collector number gets that exact printing, and a misspelt name the closest card. Or download every card a Scryfall search matches, such as set:mh2 t:creature pow>=3.",
  },
  {
    icon: LuPackage,
    title: "Precons and EDHREC lists",
    body: "Pick any preconstructed deck MTGJSON lists, by a picture of its box, and get the art of the exact printings in it. Search them in Scryfall's words. Presets give EDHREC's most-played Commander cards.",
  },
  {
    icon: LuFolderTree,
    title: "Files named your way",
    body: "Names come from a template, NAME (ARTIST) [SET] {NUMBER} by default, with folders for each source and layout. A double-faced card is saved as one image per face.",
  },
  {
    icon: LuZap,
    title: "Quick, and easy on the sites",
    body: "Cards are looked up in a local copy of Scryfall's card data, refreshed daily. The app keeps to Scryfall's rate limits and remembers where it found each printing's art.",
  },
];

function Home() {
  return (
    <>
      <Container maxW="6xl" pt={{ base: 12, md: 20 }} pb={{ base: 10, md: 16 }}>
        <Stack gap={6} align="center" textAlign="center" maxW="3xl" mx="auto">
          <Heading as="h1" size={{ base: "4xl", md: "6xl" }} letterSpacing="tight">
            Card art for a whole deck, in one go
          </Heading>
          <Text fontSize={{ base: "lg", md: "xl" }} color="fg.muted">
            Paste a deck list, run a Scryfall search, or pick an EDHREC list or a preconstructed
            deck, and MTG Art Downloader saves the art for every card. A desktop app for Windows,
            macOS and Linux, and a command line tool.
          </Text>
          <HStack gap={3} wrap="wrap" justify="center">
            <LinkButton asChild size="lg">
              <RouterLink to="/download">
                <LuDownload /> Download
              </RouterLink>
            </LinkButton>
            <LinkButton
              href={GITHUB_REPO_URL}
              target="_blank"
              rel="noreferrer"
              variant="outline"
              size="lg"
            >
              View on GitHub
            </LinkButton>
          </HStack>
        </Stack>
      </Container>

      <Container maxW="5xl">
        <Screenshot />
      </Container>

      <Container maxW="6xl" py={{ base: 14, md: 20 }}>
        <SimpleGrid columns={{ base: 1, md: 2, lg: 3 }} gap={6}>
          {FEATURES.map(({ icon, title, body }) => (
            <Card
              key={title}
              variant="subtle"
              icon={<Icon as={icon} boxSize={6} color="emerald.fg" aria-hidden />}
              title={title}
              titleProps={{ as: "h2" }}
              description={body}
            />
          ))}
        </SimpleGrid>
      </Container>

      <Container maxW="4xl">
        <Card
          size={{ base: "md", md: "lg" }}
          title={
            <>
              <Icon as={LuTerminal} boxSize={6} color="emerald.fg" aria-hidden />
              From the command line
            </>
          }
          titleProps={{ as: "h2", display: "flex", alignItems: "center", gap: 3 }}
          description={
            <>
              <Code>mtg-art</Code> does everything the app does, and reads the same settings.
            </>
          }
          bodyProps={{ gap: 4 }}
        >
          {/* A line too long for a phone scrolls sideways. */}
          <ScrollArea orientation="horizontal" rounded="md" bg="bg.muted">
            <Box as="pre" p={4} fontSize="sm">
              <code>
                {[
                  "mtg-art cards.txt                       # a card list file",
                  'mtg-art --search "set:mh2 t:creature"   # a Scryfall search',
                  "mtg-art --preset edhrec-top-year        # a preset; --presets lists them",
                  "mtg-art --deck AbzanArmor_TDC           # a precon, by its file's name on MTGJSON",
                  "mtg-art cards.txt --out ./art           # somewhere other than the settings' folder",
                ].join("\n")}
              </code>
            </Box>
          </ScrollArea>
        </Card>
      </Container>

      <Container maxW="3xl" pt={{ base: 14, md: 20 }}>
        <Text textAlign="center" color="fg.muted" fontSize="sm">
          A rewrite of{" "}
          <Link
            href="https://github.com/MrTeferi/MTG-Art-Downloader"
            target="_blank"
            rel="noreferrer"
          >
            MTG Art Downloader
          </Link>{" "}
          by Mr Teferi, by way of{" "}
          <Link
            href="https://github.com/alex-taxiera/mtg-art-downloader"
            target="_blank"
            rel="noreferrer"
          >
            Alex Taxiera's fork
          </Link>
          .
        </Text>
      </Container>
    </>
  );
}
