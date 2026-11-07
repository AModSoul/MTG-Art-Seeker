import {
  Button,
  CloseButton,
  Code,
  Drawer,
  Field,
  Fieldset,
  HStack,
  IconButton,
  Input,
  NativeSelect,
  Portal,
  SegmentGroup,
  Stack,
  Switch,
  Text,
} from "@chakra-ui/react";
import { open } from "@tauri-apps/plugin-dialog";
import type { ReactNode } from "react";
import { LuFolderOpen, LuSettings } from "react-icons/lu";

import type { Printing } from "@/bindings/Printing";
import type { ScryfallImage } from "@/bindings/ScryfallImage";
import type { Settings } from "@/bindings/Settings";
import type { SourceMode } from "@/bindings/SourceMode";
import { useSettings } from "@/store";

const SOURCES: { value: SourceMode; label: string }[] = [
  { value: "mtgpicsThenScryfall", label: "MTGPics, then Scryfall" },
  { value: "mtgpicsOnly", label: "MTGPics only" },
  { value: "scryfallOnly", label: "Scryfall only" },
];

const SCRYFALL_IMAGES: { value: ScryfallImage; label: string }[] = [
  { value: "artCrop", label: "Art crop (JPEG)" },
  { value: "fullCard", label: "Full card scan (PNG)" },
];

const PRINTINGS: { value: Printing; label: string }[] = [
  { value: "newest", label: "Newest" },
  { value: "oldest", label: "Oldest" },
];

export function SettingsDrawer() {
  const settings = useSettings((state) => state.settings);
  const update = useSettings((state) => state.update);

  return (
    <Drawer.Root size="md">
      <Drawer.Trigger asChild>
        <IconButton variant="ghost" size="sm" aria-label="Settings" disabled={!settings}>
          <LuSettings />
        </IconButton>
      </Drawer.Trigger>
      <Portal>
        <Drawer.Backdrop />
        <Drawer.Positioner>
          <Drawer.Content>
            <Drawer.Header>
              <Drawer.Title>Settings</Drawer.Title>
            </Drawer.Header>
            <Drawer.Body>
              {settings && <SettingsForm settings={settings} update={update} />}
            </Drawer.Body>
            <Drawer.CloseTrigger asChild>
              <CloseButton size="sm" />
            </Drawer.CloseTrigger>
          </Drawer.Content>
        </Drawer.Positioner>
      </Portal>
    </Drawer.Root>
  );
}

function SettingsForm({
  settings,
  update,
}: {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
}) {
  async function chooseFolder() {
    const dir = await open({ directory: true, defaultPath: settings.outputDir });
    if (typeof dir === "string") update({ outputDir: dir });
  }

  return (
    <Stack gap="8" pb="6">
      <Text fontSize="sm" color="fg.muted">
        Changes are saved straight away and apply to the next download.
      </Text>

      <Fieldset.Root>
        <Fieldset.Legend>Files</Fieldset.Legend>
        <Fieldset.Content>
          <Field.Root>
            <Field.Label>Save art to</Field.Label>
            <HStack w="full">
              <Input value={settings.outputDir} readOnly flex="1" />
              <Button variant="outline" onClick={() => void chooseFolder()}>
                <LuFolderOpen /> Choose…
              </Button>
            </HStack>
          </Field.Root>
          <Field.Root>
            <Field.Label>File names</Field.Label>
            <Input
              value={settings.naming}
              onChange={(event) => update({ naming: event.target.value })}
              fontFamily="mono"
              spellCheck={false}
            />
            <Field.HelperText>
              <Code size="sm">NAME</Code>, <Code size="sm">ARTIST</Code>, <Code size="sm">SET</Code>{" "}
              and <Code size="sm">NUMBER</Code> are filled in. For example:{" "}
              <Text as="span">{namePreview(settings.naming)}</Text>
            </Field.HelperText>
          </Field.Root>
          <Toggle
            label="Replace files with the same name"
            help="Otherwise a new file is numbered: Name (1).jpg"
            checked={settings.overwrite}
            onChange={(overwrite) => update({ overwrite })}
          />
          <Toggle
            label="Separate MTGPics and Scryfall folders"
            checked={settings.sourceFolders}
            onChange={(sourceFolders) => update({ sourceFolders })}
          />
          <Toggle
            label="A folder for each layout"
            help="MDFC Front, MDFC Back, Adventure, Saga, Basic…"
            checked={settings.layoutFolders}
            onChange={(layoutFolders) => update({ layoutFolders })}
          />
          <Toggle
            label="A folder for each card type"
            help="Creature, Land, Instant…"
            checked={settings.typeFolders}
            onChange={(typeFolders) => update({ typeFolders })}
          />
        </Fieldset.Content>
      </Fieldset.Root>

      <Fieldset.Root>
        <Fieldset.Legend>Sources</Fieldset.Legend>
        <Fieldset.Content>
          <Field.Root>
            <Field.Label>Where art comes from</Field.Label>
            <NativeSelect.Root>
              <NativeSelect.Field
                value={settings.sources}
                onChange={(event) => {
                  const sources = SOURCES.find((s) => s.value === event.target.value);
                  if (sources) update({ sources: sources.value });
                }}
              >
                {SOURCES.map((source) => (
                  <option key={source.value} value={source.value}>
                    {source.label}
                  </option>
                ))}
              </NativeSelect.Field>
              <NativeSelect.Indicator />
            </NativeSelect.Root>
            <Field.HelperText>
              MTGPics has larger, uncropped art; Scryfall has every card.
            </Field.HelperText>
          </Field.Root>
          <Field.Root>
            <Field.Label>Scryfall image</Field.Label>
            <NativeSelect.Root>
              <NativeSelect.Field
                value={settings.scryfallImage}
                onChange={(event) => {
                  const image = SCRYFALL_IMAGES.find((i) => i.value === event.target.value);
                  if (image) update({ scryfallImage: image.value });
                }}
              >
                {SCRYFALL_IMAGES.map((image) => (
                  <option key={image.value} value={image.value}>
                    {image.label}
                  </option>
                ))}
              </NativeSelect.Field>
              <NativeSelect.Indicator />
            </NativeSelect.Root>
          </Field.Root>
        </Fieldset.Content>
      </Fieldset.Root>

      <Fieldset.Root>
        <Fieldset.Legend>Cards named without a set</Fieldset.Legend>
        <Fieldset.Content>
          <Field.Root>
            <Field.Label>Printing</Field.Label>
            <SegmentGroup.Root
              size="sm"
              value={settings.printing}
              onValueChange={(event) => {
                const printing = PRINTINGS.find((p) => p.value === event.value);
                if (printing) update({ printing: printing.value });
              }}
            >
              <SegmentGroup.Indicator />
              <SegmentGroup.Items items={PRINTINGS} />
            </SegmentGroup.Root>
          </Field.Root>
          <Toggle
            label="Download every printing's art"
            help="Instead of one printing per card"
            checked={settings.downloadAll}
            onChange={(downloadAll) => update({ downloadAll })}
          />
        </Fieldset.Content>
      </Fieldset.Root>

      <Fieldset.Root>
        <Fieldset.Legend>Searching</Fieldset.Legend>
        <Fieldset.Content>
          <Toggle
            label="One printing per artwork"
            help="Skip reprints that reuse the same art"
            checked={settings.uniqueArt}
            onChange={(uniqueArt) => update({ uniqueArt })}
          />
          <Toggle
            label="Skip full-art printings"
            checked={settings.excludeFullArt}
            onChange={(excludeFullArt) => update({ excludeFullArt })}
          />
          <Toggle
            label="Include extras"
            help="Un-cards, oversized cards, championship cards and the like"
            checked={settings.includeExtras}
            onChange={(includeExtras) => update({ includeExtras })}
          />
        </Fieldset.Content>
      </Fieldset.Root>
    </Stack>
  );
}

function Toggle({
  label,
  help,
  checked,
  onChange,
}: {
  label: string;
  help?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <Stack gap="0.5">
      <Switch.Root checked={checked} onCheckedChange={(event) => onChange(event.checked)}>
        <Switch.HiddenInput />
        <Switch.Control />
        <Switch.Label>{label}</Switch.Label>
      </Switch.Root>
      {help && (
        <Text fontSize="xs" color="fg.muted" ps="11">
          {help}
        </Text>
      )}
    </Stack>
  );
}

const SAMPLE = { NAME: "Karn, Scion of Urza", ARTIST: "Chase Stone", SET: "DOM", NUMBER: "1" };

/** What a template makes of a sample card. The Rust side has the final say:
 * it also removes characters a file name can't have. */
function namePreview(template: string): string {
  const filled = template.replace(/NAME|ARTIST|SET|NUMBER/g, (placeholder) => {
    switch (placeholder) {
      case "NAME":
        return SAMPLE.NAME;
      case "ARTIST":
        return SAMPLE.ARTIST;
      case "SET":
        return SAMPLE.SET;
      default:
        return SAMPLE.NUMBER;
    }
  });
  return `${filled.trim() || "card"}.jpg`;
}
