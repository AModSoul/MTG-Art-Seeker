import {
  Box,
  Button,
  Center,
  HStack,
  IconButton,
  Image,
  Input,
  SimpleGrid,
  Stack,
  Text,
} from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { LuPackage, LuX } from "react-icons/lu";

import { api } from "@/api";
import type { DeckSearch } from "@/bindings/DeckSearch";
import type { DeckSummary } from "@/bindings/DeckSummary";
import { DataListItem, DataListRoot } from "@mtg-art/ui/data-list";
import { RadioCardItem, RadioCardLabel, RadioCardRoot } from "@mtg-art/ui/radio-card";
import { ScrollArea } from "@mtg-art/ui/scroll-area";
import { Skeleton } from "@mtg-art/ui/skeleton";
import { Tooltip } from "@mtg-art/ui/tooltip";
import { errorText } from "@/notify";
import { type PickedDeck, useInput } from "@/store";

/** How many decks are shown at first, and how many more each "Show more" adds. */
export const DECK_PAGE = 40;

/** How many placeholder tiles stand in for the decks while MTGJSON's list is fetched. */
const PLACEHOLDER_TILES = 6;

/** The height of a deck's box picture. */
const BOX_HEIGHT = "40";

/** Every precon MTGJSON lists, two to a row with a picture of its box, and a
 * search above them in Scryfall's words. */
export function DeckPicker() {
  const { deckQuery, deck, setInput } = useInput();
  const [limit, setLimit] = useState(DECK_PAGE);
  const found = useDeckSearch(deckQuery, limit);

  return (
    <Stack gap="3" flex="1" minH="0">
      <Input
        value={deckQuery}
        onChange={(event) => {
          setLimit(DECK_PAGE);
          setInput({ deckQuery: event.target.value });
        }}
        placeholder="set:eoc   t:commander   year>=2024"
        aria-label="Search the precon decks"
        fontFamily="mono"
        spellCheck={false}
      />
      <SearchSummary found={found} />
      {/* Room at the end for the scrollbar, so it doesn't cover the tiles. */}
      <ScrollArea flex="1" minH="0" mx="-1" contentProps={{ ps: "1", pe: "3" }}>
        {!found.search && !found.error && <PlaceholderTiles />}
        {found.search && found.search.decks.length > 0 && (
          <Stack gap="3" pb="1">
            <RadioCardRoot
              value={deck?.fileName ?? null}
              onValueChange={(event) => {
                const summary = found.search?.decks.find((d) => d.fileName === event.value);
                if (!summary) return;
                setInput({
                  deck: {
                    fileName: summary.fileName,
                    name: summary.name,
                    code: summary.code,
                    setName: summary.setName,
                  },
                  deckFolder: summary.name,
                });
              }}
              orientation="vertical"
              size="sm"
            >
              <RadioCardLabel srOnly>Precon decks</RadioCardLabel>
              <SimpleGrid columns={2} gap="2">
                {found.search.decks.map((summary) => (
                  <DeckTile key={summary.fileName} deck={summary} />
                ))}
              </SimpleGrid>
            </RadioCardRoot>
            {found.search.total > found.search.decks.length && (
              <Button
                size="sm"
                variant="outline"
                loading={found.loading}
                onClick={() => setLimit((shown) => shown + DECK_PAGE)}
              >
                Show more
              </Button>
            )}
          </Stack>
        )}
      </ScrollArea>
      {deck && <PickedDeckLine deck={deck} onClear={() => setInput({ deck: null })} />}
    </Stack>
  );
}

interface Found {
  search: DeckSearch | null;
  loading: boolean;
  error: string | null;
}

function SearchSummary({ found }: { found: Found }) {
  if (found.error) {
    return (
      <Text fontSize="sm" color="fg.error">
        {found.error}
      </Text>
    );
  }
  if (!found.search) {
    return (
      <Text fontSize="sm" color="fg.muted">
        Getting the list of decks from MTGJSON…
      </Text>
    );
  }
  const { total } = found.search;
  return (
    <Text fontSize="sm" color="fg.muted">
      {total === 0
        ? "No decks match"
        : `${total.toLocaleString()} ${total === 1 ? "deck" : "decks"}`}
    </Text>
  );
}

/** Tiles the shape of the decks', while MTGJSON's list is fetched: a box over
 * two lines of text, in a card's border. */
function PlaceholderTiles() {
  return (
    <SimpleGrid columns={2} gap="2" pb="1" aria-hidden>
      {Array.from({ length: PLACEHOLDER_TILES }, (_, index) => (
        <Stack key={index} gap="1.5" p="3" borderWidth="1px" rounded="l2">
          <Skeleton h={BOX_HEIGHT} rounded="sm" />
          <Skeleton h="4" w="80%" />
          <Skeleton h="4" w="60%" />
        </Stack>
      ))}
    </SimpleGrid>
  );
}

/** A deck to pick: its box, its name and its set, and what kind of deck it is
 * on hover. */
function DeckTile({ deck }: { deck: DeckSummary }) {
  return (
    <Tooltip content={deck.type}>
      {/* The tooltip's anchor: the item hands its ref to its hidden input. */}
      <Box display="flex">
        <RadioCardItem
          value={deck.fileName}
          flex="1"
          cursor="pointer"
          indicator={null}
          icon={<DeckBox deck={deck} />}
          label={
            <Text as="span" lineClamp={2}>
              {deck.name}
            </Text>
          }
          description={
            <Text as="span" lineClamp={2}>
              {deck.setName}
            </Text>
          }
        />
      </Box>
    </Tooltip>
  );
}

/** Pictures already asked for, by deck: a path, or `null` for none. Kept
 * across searches so a deck seen once isn't asked about again. */
const boxes = new Map<string, string | null>();

/** The picture of a deck's box, asked for once it scrolls into view, and held
 * by a placeholder its size until it has loaded. */
function DeckBox({ deck }: { deck: DeckSummary }) {
  const ref = useRef<HTMLDivElement>(null);
  const known = boxes.get(deck.fileName);
  const [path, setPath] = useState<string | null | undefined>(deck.hasBox ? known : null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    if (path !== undefined || !ref.current) return;
    let current = true;
    const observer = new IntersectionObserver((entries) => {
      if (!entries.some((entry) => entry.isIntersecting)) return;
      observer.disconnect();
      api.deckBox(deck.fileName).then(
        (found) => {
          boxes.set(deck.fileName, found);
          if (current) setPath(found);
        },
        () => current && setPath(null),
      );
    });
    observer.observe(ref.current);
    return () => {
      current = false;
      observer.disconnect();
    };
  }, [deck.fileName, path]);

  return (
    <Skeleton
      ref={ref}
      loading={path === undefined || (path !== null && !loaded)}
      h={BOX_HEIGHT}
      w="full"
      rounded="sm"
    >
      <Center h="full" rounded="sm" bg={path ? undefined : "bg.subtle"}>
        {path ? (
          <Image
            src={convertFileSrc(path)}
            alt={`The box of ${deck.name}`}
            maxH="full"
            maxW="full"
            objectFit="contain"
            onLoad={() => setLoaded(true)}
            // A picture that won't load is no picture.
            onError={() => setPath(null)}
          />
        ) : (
          <Box color="fg.subtle" fontSize="3xl" aria-hidden>
            <LuPackage />
          </Box>
        )}
      </Center>
    </Skeleton>
  );
}

function PickedDeckLine({ deck, onClear }: { deck: PickedDeck; onClear: () => void }) {
  return (
    <HStack gap="2" borderTopWidth="1px" pt="2">
      <DataListRoot flex="1" minW="0">
        <DataListItem
          label="Picked"
          value={
            <Tooltip content={`${deck.name} · ${deck.setName}`}>
              <Text fontWeight="medium" truncate>
                {deck.name}{" "}
                <Text as="span" color="fg.muted" fontWeight="normal">
                  ({deck.code})
                </Text>
              </Text>
            </Tooltip>
          }
        />
      </DataListRoot>
      <IconButton size="xs" variant="ghost" aria-label="Unpick the deck" onClick={onClear}>
        <LuX />
      </IconButton>
    </HStack>
  );
}

/** The decks `query` matches, asked for a moment after typing stops. The last
 * answer stays shown while the next is on its way. */
function useDeckSearch(query: string, limit: number): Found {
  const asked = `${limit}:${query.trim()}`;
  const [answer, setAnswer] = useState<{
    asked: string;
    search: DeckSearch | null;
    error: string | null;
  } | null>(null);
  useEffect(() => {
    let current = true;
    const timer = setTimeout(() => {
      api.searchDecks(query.trim(), limit).then(
        (search) => current && setAnswer({ asked, search, error: null }),
        (error: unknown) => current && setAnswer({ asked, search: null, error: errorText(error) }),
      );
    }, 200);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [asked, query, limit]);
  return {
    search: answer?.search ?? null,
    error: answer?.error ?? null,
    loading: answer?.asked !== asked,
  };
}
