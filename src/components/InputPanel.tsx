import {
  Button,
  Field,
  Flex,
  HStack,
  IconButton,
  Input,
  RadioGroup,
  Stack,
  Tabs,
  Text,
  Textarea,
} from "@chakra-ui/react";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { LuDownload, LuFileText, LuFolderOpen, LuSquare } from "react-icons/lu";

import { api, errorText } from "@/api";
import type { ParsedList } from "@/bindings/ParsedList";
import type { PresetInfo } from "@/bindings/PresetInfo";
import type { RunInput } from "@/bindings/RunInput";
import { toaster } from "@/components/ui/toaster";
import { cancelRun, isInputTab, startRun, useInput, useRun, useSettings } from "@/store";

const LIST_PLACEHOLDER = `Paste a card list, one card per line:

Damnation
4 Lightning Bolt
Karn, Scion of Urza (DOM) 1
1 Sol Ring (C21) 263 *F*`;

export function InputPanel() {
  const { tab, text, query, presetId, setInput } = useInput();
  const running = useRun((state) => state.running);
  const parsed = useParsedList(text);
  const presets = usePresets();
  const [count, setCount] = useState<{ query: string; total: number } | null>(null);
  const [counting, setCounting] = useState(false);
  const settings = useSettings((state) => state.settings);

  const input: RunInput | null =
    tab === "list"
      ? parsed && parsed.cards.length > 0
        ? { kind: "list", text }
        : null
      : tab === "search"
        ? query.trim()
          ? { kind: "search", query: query.trim() }
          : null
        : presetId
          ? { kind: "preset", id: presetId }
          : null;

  async function openList() {
    const path = await open({
      multiple: false,
      filters: [
        { name: "Card lists", extensions: ["txt", "dec", "dek", "csv", "md"] },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (typeof path !== "string") return;
    try {
      setInput({ text: await api.readCardList(path) });
    } catch (error) {
      toaster.create({
        type: "error",
        title: "Couldn't open the list",
        description: errorText(error),
      });
    }
  }

  async function countMatches() {
    if (!settings || !query.trim()) return;
    setCounting(true);
    try {
      setCount({ query, total: await api.countSearch(query.trim(), settings) });
    } catch (error) {
      toaster.create({ type: "error", title: "Search failed", description: errorText(error) });
    } finally {
      setCounting(false);
    }
  }

  return (
    <Flex direction="column" h="full" p="5" gap="4">
      <Tabs.Root
        value={tab}
        onValueChange={(event) => isInputTab(event.value) && setInput({ tab: event.value })}
        variant="enclosed"
        fitted
        flex="1"
        display="flex"
        flexDirection="column"
        minH="0"
      >
        <Tabs.List>
          <Tabs.Trigger value="list">Card list</Tabs.Trigger>
          <Tabs.Trigger value="search">Search</Tabs.Trigger>
          <Tabs.Trigger value="preset">Presets</Tabs.Trigger>
        </Tabs.List>

        <Tabs.Content value="list" flex="1" display="flex" flexDirection="column" gap="2" minH="0">
          <Textarea
            value={text}
            onChange={(event) => setInput({ text: event.target.value })}
            placeholder={LIST_PLACEHOLDER}
            fontFamily="mono"
            fontSize="sm"
            flex="1"
            minH="60"
            resize="none"
            spellCheck={false}
          />
          <HStack justify="space-between">
            <Text fontSize="sm" color="fg.muted">
              {listSummary(parsed)}
            </Text>
            <Button size="xs" variant="outline" onClick={() => void openList()}>
              <LuFileText /> Open file…
            </Button>
          </HStack>
        </Tabs.Content>

        <Tabs.Content value="search">
          <Stack gap="3">
            <Field.Root>
              <Field.Label>Scryfall search</Field.Label>
              <Input
                value={query}
                onChange={(event) => setInput({ query: event.target.value })}
                onKeyDown={(event) => event.key === "Enter" && void countMatches()}
                placeholder="set:mh2 t:creature pow>=3"
                fontFamily="mono"
                spellCheck={false}
              />
              <Field.HelperText>
                Anything Scryfall&apos;s search takes. Every matching card is downloaded.
              </Field.HelperText>
            </Field.Root>
            <HStack>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void countMatches()}
                loading={counting}
                disabled={!query.trim()}
              >
                Count matches
              </Button>
              {count && count.query === query && (
                <Text fontSize="sm" color="fg.muted">
                  {count.total.toLocaleString()}{" "}
                  {count.total === 1 ? "card matches" : "cards match"}
                </Text>
              )}
            </HStack>
          </Stack>
        </Tabs.Content>

        <Tabs.Content value="preset">
          <RadioGroup.Root
            value={presetId}
            onValueChange={(event) => event.value && setInput({ presetId: event.value })}
          >
            <Stack gap="3">
              <Text fontSize="sm" color="fg.muted">
                Lists of the most-played Commander cards, from EDHREC.
              </Text>
              {presets.map((preset) => (
                <RadioGroup.Item key={preset.id} value={preset.id}>
                  <RadioGroup.ItemHiddenInput />
                  <RadioGroup.ItemIndicator />
                  <RadioGroup.ItemText>{preset.label}</RadioGroup.ItemText>
                </RadioGroup.Item>
              ))}
            </Stack>
          </RadioGroup.Root>
        </Tabs.Content>
      </Tabs.Root>

      <OutputFolder />

      {running ? (
        <Button colorPalette="red" variant="subtle" onClick={cancelRun}>
          <LuSquare /> Cancel
        </Button>
      ) : (
        <Button colorPalette="blue" disabled={!input} onClick={() => input && void startRun(input)}>
          <LuDownload /> Download art
        </Button>
      )}
    </Flex>
  );
}

function OutputFolder() {
  const settings = useSettings((state) => state.settings);
  const update = useSettings((state) => state.update);
  if (!settings) return null;

  async function choose() {
    const dir = await open({ directory: true, defaultPath: settings?.outputDir });
    if (typeof dir === "string") update({ outputDir: dir });
  }

  return (
    <Stack gap="1">
      <Text fontSize="xs" color="fg.muted" fontWeight="medium">
        Saving to
      </Text>
      <HStack gap="2">
        <Text fontSize="sm" truncate flex="1" title={settings.outputDir}>
          {settings.outputDir}
        </Text>
        <IconButton
          size="xs"
          variant="ghost"
          aria-label="Choose the folder art is saved to"
          onClick={() => void choose()}
        >
          <LuFolderOpen />
        </IconButton>
      </HStack>
    </Stack>
  );
}

function listSummary(parsed: ParsedList | null): string {
  if (!parsed || (parsed.cards.length === 0 && parsed.skipped.length === 0)) return "No cards yet";
  const cards = `${parsed.cards.length} ${parsed.cards.length === 1 ? "card" : "cards"}`;
  if (parsed.skipped.length === 0) return cards;
  const skipped = parsed.skipped.length;
  return `${cards} · ${skipped} ${skipped === 1 ? "heading or comment" : "headings and comments"} skipped`;
}

/** The list as the Rust parser reads it, re-read a moment after typing stops. */
function useParsedList(text: string): ParsedList | null {
  const [parsed, setParsed] = useState<ParsedList | null>(null);
  useEffect(() => {
    let current = true;
    const timer = setTimeout(() => {
      api.parseCardList(text).then(
        (result) => current && setParsed(result),
        () => current && setParsed(null),
      );
    }, 200);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [text]);
  return parsed;
}

function usePresets(): PresetInfo[] {
  const [presets, setPresets] = useState<PresetInfo[]>([]);
  useEffect(() => {
    api.listPresets().then(setPresets, () => setPresets([]));
  }, []);
  return presets;
}
