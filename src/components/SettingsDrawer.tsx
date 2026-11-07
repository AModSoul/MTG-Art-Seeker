import {
  Button,
  Code,
  Fieldset,
  HStack,
  IconButton,
  Input,
  Span,
  Stack,
  Text,
} from "@chakra-ui/react";
import { LuFolderOpen, LuSettings } from "react-icons/lu";

import type { Printing } from "@/bindings/Printing";
import type { ScryfallImage } from "@/bindings/ScryfallImage";
import type { Settings } from "@/bindings/Settings";
import type { SourceMode } from "@/bindings/SourceMode";
import {
  DrawerBody,
  DrawerCloseTrigger,
  DrawerContent,
  DrawerHeader,
  DrawerRoot,
  DrawerTitle,
  DrawerTrigger,
} from "@/components/ui/drawer";
import { Field } from "@/components/ui/field";
import { NativeSelectField, NativeSelectRoot } from "@/components/ui/native-select";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Switch } from "@/components/ui/switch";
import { Tooltip } from "@/components/ui/tooltip";
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
  "files" | "printings" | "searching",
  { key: BooleanSetting; label: string; help?: string }[]
> = {
  files: [
    {
      key: "overwrite",
      label: "Replace files with the same name",
      help: "Otherwise a new file is numbered: Name (1).jpg",
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
  [K in keyof Settings]: Settings[K] extends boolean ? K : never;
}[keyof Settings];

export function SettingsDrawer() {
  const settings = useSettings((state) => state.settings);

  return (
    <DrawerRoot size="md">
      <Tooltip content="Settings">
        <DrawerTrigger asChild>
          <IconButton variant="ghost" size="sm" aria-label="Settings" disabled={!settings}>
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
      <Field
        key={key}
        // Lined up under the switch's label, not its track.
        helperText={help && <Span ps="11">{help}</Span>}
      >
        <Switch
          checked={settings[key]}
          onCheckedChange={(event) => {
            const patch: Partial<Settings> = {};
            patch[key] = event.checked;
            update(patch);
          }}
        >
          {label}
        </Switch>
      </Field>
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
