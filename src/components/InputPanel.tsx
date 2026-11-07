import {
  Button,
  Flex,
  HStack,
  IconButton,
  Input,
  Stack,
  Tabs,
  Text,
  Textarea,
} from "@chakra-ui/react";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { LuDownload, LuFileText, LuFolderOpen, LuSquare } from "react-icons/lu";

import { api } from "@/api";
import type { ParsedList } from "@/bindings/ParsedList";
import type { PresetInfo } from "@/bindings/PresetInfo";
import type { RunInput } from "@/bindings/RunInput";
import { Field } from "@/components/ui/field";
import { Radio, RadioGroup } from "@/components/ui/radio";
import { Tooltip } from "@/components/ui/tooltip";
import { showError } from "@/notify";
import {
  cancelRun,
  chooseOutputDir,
  type InputTab,
  startRun,
  useInput,
  useRun,
  useSettings,
} from "@/store";
import { findOption, type Option } from "@/utils/options";

const TABS: Option<InputTab>[] = [
  { value: "list", label: "Card list" },
  { value: "search", label: "Search" },
  { value: "preset", label: "Presets" },
];

const LIST_PLACEHOLDER = `Paste a card list, one card per line:

Damnation
4 Lightning Bolt
Karn, Scion of Urza (DOM) 1
1 Sol Ring (C21) 263 *F*`;

export function InputPanel() {
  const { tab, text, query, presetId, setInput } = useInput();
  const running = useRun((state) => state.running);
  const parsed = useParsedList(text);

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

  return (
    <Flex direction="column" h="full" p="5" gap="4">
      <Tabs.Root
        value={tab}
        onValueChange={(event) => {
          const chosen = findOption(TABS, event.value);
          if (chosen) setInput({ tab: chosen });
        }}
        variant="enclosed"
        fitted
        flex="1"
        display="flex"
        flexDirection="column"
        minH="0"
      >
        <Tabs.List>
          {TABS.map((option) => (
            <Tabs.Trigger key={option.value} value={option.value}>
              {option.label}
            </Tabs.Trigger>
          ))}
        </Tabs.List>

        <Tabs.Content value="list" flex="1" display="flex" flexDirection="column" gap="2" minH="0">
          <ListInput text={text} parsed={parsed} onChange={(text) => setInput({ text })} />
        </Tabs.Content>
        <Tabs.Content value="search">
          <SearchInput query={query} onChange={(query) => setInput({ query })} />
        </Tabs.Content>
        <Tabs.Content value="preset">
          <PresetInput presetId={presetId} onChange={(presetId) => setInput({ presetId })} />
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

function ListInput({
  text,
  parsed,
  onChange,
}: {
  text: string;
  parsed: ParsedList | null;
  onChange: (text: string) => void;
}) {
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
      onChange(await api.readCardList(path));
    } catch (error) {
      showError("Couldn't open the list", error);
    }
  }

  return (
    <>
      <Textarea
        value={text}
        onChange={(event) => onChange(event.target.value)}
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
    </>
  );
}

function SearchInput({ query, onChange }: { query: string; onChange: (query: string) => void }) {
  const settings = useSettings((state) => state.settings);
  const [count, setCount] = useState<{ query: string; total: number } | null>(null);
  const [counting, setCounting] = useState(false);

  async function countMatches() {
    if (!settings || !query.trim()) return;
    setCounting(true);
    try {
      setCount({ query, total: await api.countSearch(query.trim(), settings) });
    } catch (error) {
      showError("Search failed", error);
    } finally {
      setCounting(false);
    }
  }

  return (
    <Stack gap="3">
      <Field
        label="Scryfall search"
        helperText="Anything Scryfall's search takes. Every matching card is downloaded."
      >
        <Input
          value={query}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && void countMatches()}
          placeholder="set:mh2 t:creature pow>=3"
          fontFamily="mono"
          spellCheck={false}
        />
      </Field>
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
            {count.total.toLocaleString()} {count.total === 1 ? "card matches" : "cards match"}
          </Text>
        )}
      </HStack>
    </Stack>
  );
}

function PresetInput({
  presetId,
  onChange,
}: {
  presetId: string;
  onChange: (presetId: string) => void;
}) {
  const presets = usePresets();
  return (
    <RadioGroup value={presetId} onValueChange={(event) => event.value && onChange(event.value)}>
      <Stack gap="3">
        <Text fontSize="sm" color="fg.muted">
          Lists of the most-played Commander cards, from EDHREC.
        </Text>
        {presets.map((preset) => (
          <Radio key={preset.id} value={preset.id}>
            {preset.label}
          </Radio>
        ))}
      </Stack>
    </RadioGroup>
  );
}

function OutputFolder() {
  const outputDir = useSettings((state) => state.settings?.outputDir);
  if (!outputDir) return null;

  return (
    <Stack gap="1">
      <Text fontSize="xs" color="fg.muted" fontWeight="medium">
        Saving to
      </Text>
      <HStack gap="2">
        <Text fontSize="sm" truncate flex="1" title={outputDir}>
          {outputDir}
        </Text>
        <Tooltip content="Choose a folder">
          <IconButton
            size="xs"
            variant="ghost"
            aria-label="Choose the folder art is saved to"
            onClick={() => void chooseOutputDir()}
          >
            <LuFolderOpen />
          </IconButton>
        </Tooltip>
      </HStack>
    </Stack>
  );
}

function listSummary(parsed: ParsedList | null): string {
  if (!parsed || (parsed.cards.length === 0 && parsed.skipped.length === 0)) return "No cards yet";
  const cards = `${parsed.cards.length} ${parsed.cards.length === 1 ? "card" : "cards"}`;
  const skipped = parsed.skipped.length;
  if (skipped === 0) return cards;
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
