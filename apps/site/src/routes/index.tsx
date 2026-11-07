import {
  Box,
  Button,
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
import { createFileRoute, Link as RouterLink } from "@tanstack/react-router";
import type { IconType } from "react-icons";
import {
  LuDownload,
  LuFileText,
  LuFolderTree,
  LuImage,
  LuSearch,
  LuTerminal,
  LuTrophy,
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
    icon: LuFileText,
    title: "Any deck list",
    body: "Paste or open the lists deck sites export. A line with a set and collector number gets that exact printing; a misspelt name is matched to the closest card.",
  },
  {
    icon: LuSearch,
    title: "Scryfall searches",
    body: "Anything Scryfall's search takes, such as set:mh2 t:creature pow>=3, downloads every matching card. Count the matches first.",
  },
  {
    icon: LuTrophy,
    title: "EDHREC top lists",
    body: "Presets for EDHREC's most-played Commander cards.",
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
            Paste a deck list, run a Scryfall search, or pick an EDHREC list, and MTG Art Downloader
            saves the art for every card. A desktop app for Windows, macOS and Linux, and a command
            line tool.
          </Text>
          <HStack gap={3} wrap="wrap" justify="center">
            <Button asChild colorPalette="blue" size="lg">
              <RouterLink to="/download">
                <LuDownload /> Download
              </RouterLink>
            </Button>
            <Button asChild variant="outline" size="lg">
              <a href={GITHUB_REPO_URL} target="_blank" rel="noreferrer">
                View on GitHub
              </a>
            </Button>
          </HStack>
        </Stack>
      </Container>

      <Container maxW="5xl">
        <Screenshot />
      </Container>

      <Container maxW="6xl" py={{ base: 14, md: 20 }}>
        <SimpleGrid columns={{ base: 1, md: 2, lg: 3 }} gap={6}>
          {FEATURES.map(({ icon, title, body }) => (
            <Stack key={title} gap={3} p={6} rounded="xl" borderWidth="1px" bg="bg.subtle">
              <Icon as={icon} boxSize={6} color="blue.fg" aria-hidden />
              <Heading as="h2" size="md">
                {title}
              </Heading>
              <Text color="fg.muted">{body}</Text>
            </Stack>
          ))}
        </SimpleGrid>
      </Container>

      <Container maxW="4xl">
        <Stack gap={4} p={{ base: 6, md: 8 }} rounded="xl" borderWidth="1px">
          <HStack gap={3}>
            <Icon as={LuTerminal} boxSize={6} color="blue.fg" aria-hidden />
            <Heading as="h2" size="lg">
              From the command line
            </Heading>
          </HStack>
          <Text color="fg.muted">
            <Code>mtg-art</Code> does everything the app does, and reads the same settings.
          </Text>
          <Box as="pre" p={4} rounded="md" bg="bg.muted" overflowX="auto" fontSize="sm">
            <code>
              {[
                "mtg-art cards.txt                       # a card list file",
                'mtg-art --search "set:mh2 t:creature"   # a Scryfall search',
                "mtg-art --preset edhrec-top-year        # a preset; --presets lists them",
                "mtg-art cards.txt --out ./art           # somewhere other than the settings' folder",
              ].join("\n")}
            </code>
          </Box>
        </Stack>
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
