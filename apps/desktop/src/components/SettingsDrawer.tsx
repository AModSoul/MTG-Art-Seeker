import { Button, Code, Fieldset, HStack, IconButton, Input, Stack, Text } from "@chakra-ui/react";
import { useEffect, useState } from "react";
import { LuFolderOpen, LuSettings } from "react-icons/lu";

import { api } from "@/api";
import type { CardDataStatus } from "@/bindings/CardDataStatus";
import type { Printing } from "@/bindings/Printing";
import type { ScryfallImage } from "@/bindings/ScryfallImage";
import type { Settings } from "@/bindings/Settings";
import type { SourceMode } from "@/bindings/SourceMode";
import { DataListItem, DataListRoot } from "@mtg-art/ui/data-list";
import {
  DrawerBody,
  DrawerCloseTrigger,
  DrawerContent,
  DrawerHeader,
  DrawerRoot,
  DrawerTitle,
  DrawerTrigger,
} from "@/components/ui/drawer";
import { Field } from "@mtg-art/ui/field";
import { NativeSelectField, NativeSelectRoot } from "@mtg-art/ui/native-select";
import { SegmentedControl } from "@mtg-art/ui/segmented-control";
import { SkeletonText } from "@mtg-art/ui/skeleton";
import { SwitchField } from "@mtg-art/ui/switch-field";
import { Tooltip } from "@mtg-art/ui/tooltip";
import { HEADER_BUTTON } from "@/layout";
import { chooseOutputDir, useSettings } from "@/store";
import { findOption, type Option } from "@/utils/options";

const SOURCES: Option<SourceMode>[] = [
  { value: "mtgpicsThenScryfall", label: "MTGPics, then Scryfall" },
  { value: "mtgpicsOnly", label: "MTGPics only" },
  { value: "scryfallOnly", label: "Scryfall only" },
];

const SCRYFALL_IMAGES: Option<ScryfallImage>[] = [
  { value: "artCrop", label: "Art crop (JPEG)" },
  { value: "fullCard", label: "Full card scan (PNG)" },
];

const PRINTINGS: Option<Printing>[] = [
  { value: "newest", label: "Newest" },
  { value: "oldest", label: "Oldest" },
];

/** The on/off settings, in the order they're shown, by section. */
const SWITCHES: Record<
  "sources" | "files" | "printings" | "searching",
  { key: BooleanSetting; label: string; help?: string }[]
> = {
  sources: [
    {
      key: "searchMtgpicsArt",
      label: "Search for larger copies of MTGPics' art too",
      help: "Shows the Google Lens, ArtStation and other search buttons beside MTGPics' art, not only Scryfall's.",
    },
  ],
  files: [
    {
      key: "overwrite",
      label: "Replace files with the same name",
      help: "Otherwise a new file is numbered: Name (1).jpg",
    },
    {
      key: "listFolders",
      label: "A folder for each card list",
      help: "Named under the list. Searches and presets aren't affected.",
    },
    { key: "sourceFolders", label: "Separate MTGPics and Scryfall folders" },
    {
      key: "layoutFolders",
      label: "A folder for each layout",
      help: "MDFC Front, MDFC Back, Adventure, Saga, Basic…",
    },
    { key: "typeFolders", label: "A folder for each card type", help: "Creature, Land, Instant…" },
  ],
  printings: [
    {
      key: "downloadAll",
      label: "Download every printing's art",
      help: "Instead of one printing per card",
    },
  ],
  searching: [
    {
      key: "uniqueArt",
      label: "One printing per artwork",
      help: "Skip reprints that reuse the same art",
    },
    { key: "excludeFullArt", label: "Skip full-art printings" },
    {
      key: "includeExtras",
      label: "Include extras",
      help: "Un-cards, oversized cards, championship cards and the like",
    },
  ],
};

type BooleanSetting = {
  [K in keyof Settings]-?: Settings[K] extends boolean ? K : never;
}[keyof Settings];

export function SettingsDrawer() {
  const settings = useSettings((state) => state.settings);

  return (
    <DrawerRoot size="md">
      <Tooltip content="Settings">
        <DrawerTrigger asChild>
          <IconButton {...HEADER_BUTTON} aria-label="Settings" disabled={!settings}>
            <LuSettings />
          </IconButton>
        </DrawerTrigger>
      </Tooltip>
      <DrawerContent>
        <DrawerHeader>
          <DrawerTitle>Settings</DrawerTitle>
        </DrawerHeader>
        <DrawerBody>{settings && <SettingsForm settings={settings} />}</DrawerBody>
        <DrawerCloseTrigger />
      </DrawerContent>
    </DrawerRoot>
  );
}

function SettingsForm({ settings }: { settings: Settings }) {
  const update = useSettings((state) => state.update);
  const switches = (section: keyof typeof SWITCHES) =>
    SWITCHES[section].map(({ key, label, help }) => (
      <SwitchField
        key={key}
        label={label}
        helperText={help}
        checked={settings[key]}
        onCheckedChange={(event) => {
          const patch: Partial<Settings> = {};
          patch[key] = event.checked;
          update(patch);
        }}
      />
    ));

  return (
    <Stack gap="8" pb="6">
      <Text fontSize="sm" color="fg.muted">
        Changes are saved straight away and apply to the next download.
      </Text>

      <Fieldset.Root>
        <Fieldset.Legend>Files</Fieldset.Legend>
        <Fieldset.Content>
          <Field label="Save art to">
            <HStack w="full">
              <Input value={settings.outputDir} readOnly flex="1" />
              <Button variant="outline" onClick={() => void chooseOutputDir()}>
                <LuFolderOpen /> Choose…
              </Button>
            </HStack>
          </Field>
          <Field
            label="File names"
            helperText={
              <>
                <Code size="sm">NAME</Code>, <Code size="sm">ARTIST</Code>,{" "}
                <Code size="sm">SET</Code> and <Code size="sm">NUMBER</Code> are filled in. For
                example: {namePreview(settings.naming)}
              </>
            }
          >
            <Input
              value={settings.naming}
              onChange={(event) => update({ naming: event.target.value })}
              fontFamily="mono"
              spellCheck={false}
            />
          </Field>
          {switches("files")}
        </Fieldset.Content>
      </Fieldset.Root>

      <Fieldset.Root>
        <Fieldset.Legend>Sources</Fieldset.Legend>
        <Fieldset.Content>
          <Field
            label="Where art comes from"
            helperText="MTGPics has larger, uncropped art; Scryfall has every card."
          >
            <OptionSelect
              value={settings.sources}
              options={SOURCES}
              onChange={(sources) => update({ sources })}
            />
          </Field>
          <Field label="Scryfall image">
            <OptionSelect
              value={settings.scryfallImage}
              options={SCRYFALL_IMAGES}
              onChange={(scryfallImage) => update({ scryfallImage })}
            />
          </Field>
          {switches("sources")}
          <CardData />
        </Fieldset.Content>
      </Fieldset.Root>

      <Fieldset.Root>
        <Fieldset.Legend>Cards named without a set</Fieldset.Legend>
        <Fieldset.Content>
          <Field label="Printing">
            <SegmentedControl
              size="sm"
              value={settings.printing}
              items={PRINTINGS}
              onValueChange={(event) => {
                const printing = findOption(PRINTINGS, event.value);
                if (printing) update({ printing });
              }}
            />
          </Field>
          {switches("printings")}
        </Fieldset.Content>
      </Fieldset.Root>

      <Fieldset.Root>
        <Fieldset.Legend>Searching</Fieldset.Legend>
        <Fieldset.Content>{switches("searching")}</Fieldset.Content>
      </Fieldset.Root>
    </Stack>
  );
}

/** How long between checks on the card data while the drawer is open. */
const STATUS_POLL_MS = 2000;

/** How current the local copy of Scryfall's card data is, kept up to date while shown. */
function CardData() {
  const [status, setStatus] = useState<CardDataStatus | null>(null);
  useEffect(() => {
    let current = true;
    const check = () =>
      api.cardDataStatus().then(
        (result) => current && setStatus(result),
        () => current && setStatus(null),
      );
    void check();
    const timer = setInterval(() => void check(), STATUS_POLL_MS);
    return () => {
      current = false;
      clearInterval(timer);
    };
  }, []);

  return (
    // Bold, so its label reads like the fields' around it.
    <DataListRoot variant="bold">
      <DataListItem
        label="Card data"
        value={
          status ? (
            <Stack gap="1">
              <Text>{cardDataSummary(status)}</Text>
              {status.error && <Text color="fg.error">The last update failed: {status.error}</Text>}
            </Stack>
          ) : (
            <SkeletonText noOfLines={2} />
          )
        }
      />
    </DataListRoot>
  );
}

function cardDataSummary({ updatedAt, cards, updating }: CardDataStatus): string {
  if (!updatedAt) {
    return updating
      ? "Downloading Scryfall's card data. Until it's in, each card is looked up on Scryfall."
      : "Not downloaded yet: each card is looked up on Scryfall.";
  }
  const published = new Date(updatedAt).toLocaleDateString(undefined, { dateStyle: "medium" });
  const summary = `${cards.toLocaleString()} cards from Scryfall, published ${published}. Lists are looked up here; it's updated daily.`;
  return updating ? `${summary} Downloading newer data…` : summary;
}

/** A native select over a union of strings, reporting the chosen value as that type. */
function OptionSelect<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: Option<T>[];
  onChange: (value: T) => void;
}) {
  return (
    <NativeSelectRoot>
      <NativeSelectField
        value={value}
        items={options}
        onChange={(event) => {
          const chosen = findOption(options, event.target.value);
          if (chosen) onChange(chosen);
        }}
      />
    </NativeSelectRoot>
  );
}

const SAMPLE = { NAME: "Karn, Scion of Urza", ARTIST: "Chase Stone", SET: "DOM", NUMBER: "1" };

/** What a template makes of a sample card. The Rust side has the final say:
 * it also removes characters a file name can't have. */
function namePreview(template: string): string {
  const filled = template.replace(
    /NAME|ARTIST|SET|NUMBER/g,
    (placeholder) => Object.entries(SAMPLE).find(([key]) => key === placeholder)?.[1] ?? "",
  );
  return `${filled.trim() || "card"}.jpg`;
}
