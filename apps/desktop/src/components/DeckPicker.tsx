import {
  Box,
  Button,
  Center,
  HStack,
  IconButton,
  Image,
  Input,
  SimpleGrid,
  Spinner,
  Stack,
  Text,
} from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { LuPackage, LuX } from "react-icons/lu";

import { api } from "@/api";
import type { DeckSearch } from "@/bindings/DeckSearch";
import type { DeckSummary } from "@/bindings/DeckSummary";
import { RadioCardItem, RadioCardLabel, RadioCardRoot } from "@mtg-art/ui/radio-card";
import { Tooltip } from "@mtg-art/ui/tooltip";
import { errorText } from "@/notify";
import { type PickedDeck, useInput } from "@/store";

/** How many decks are shown at first, and how many more each "Show more" adds. */
export const DECK_PAGE = 40;

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
      <Box flex="1" minH="0" overflowY="auto" mx="-1" px="1">
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
      </Box>
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
      <HStack gap="2" color="fg.muted">
        <Spinner size="xs" />
        <Text fontSize="sm">Getting the list of decks from MTGJSON…</Text>
      </HStack>
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

/** The picture of a deck's box, asked for once it scrolls into view. */
function DeckBox({ deck }: { deck: DeckSummary }) {
  const ref = useRef<HTMLDivElement>(null);
  const known = boxes.get(deck.fileName);
  const [path, setPath] = useState<string | null | undefined>(deck.hasBox ? known : null);

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
    <Center ref={ref} h="40" w="full" rounded="sm" bg={path ? undefined : "bg.subtle"}>
      {path ? (
        <Image
          src={convertFileSrc(path)}
          alt={`The box of ${deck.name}`}
          maxH="full"
          maxW="full"
          objectFit="contain"
        />
      ) : path === null ? (
        <Box color="fg.subtle" fontSize="3xl" aria-hidden>
          <LuPackage />
        </Box>
      ) : (
        <Spinner size="sm" color="fg.subtle" />
      )}
    </Center>
  );
}

function PickedDeckLine({ deck, onClear }: { deck: PickedDeck; onClear: () => void }) {
  return (
    <HStack gap="2" borderTopWidth="1px" pt="2">
      <Stack gap="0" flex="1" minW="0">
        <Text fontSize="xs" color="fg.muted">
          Picked
        </Text>
        <Tooltip content={`${deck.name} · ${deck.setName}`}>
          <Text fontSize="sm" fontWeight="medium" truncate>
            {deck.name}{" "}
            <Text as="span" color="fg.muted" fontWeight="normal">
              ({deck.code})
            </Text>
          </Text>
        </Tooltip>
      </Stack>
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
