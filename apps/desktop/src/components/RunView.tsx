import {
  Badge,
  Box,
  Button,
  Center,
  Flex,
  HStack,
  IconButton,
  Image,
  Spacer,
  Spinner,
  Stack,
  Table,
  Text,
  VisuallyHidden,
} from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { memo, useState } from "react";
import type { IconType } from "react-icons";
import {
  LuClipboardCopy,
  LuExternalLink,
  LuFolderOpen,
  LuGlobe,
  LuImageDown,
  LuImageOff,
  LuRotateCcw,
} from "react-icons/lu";
import { SiArtstation, SiDeviantart, SiGooglelens, SiInstagram, SiPixiv } from "react-icons/si";

import { api } from "@/api";
import type { ArtSource } from "@/bindings/ArtSource";
import type { JobStatus } from "@/bindings/JobStatus";
import type { KnownSource } from "@/bindings/KnownSource";
import type { SavedFile } from "@/bindings/SavedFile";
import type { Site } from "@/bindings/Site";
import { Alert } from "@mtg-art/ui/alert";
import { EmptyState } from "@mtg-art/ui/empty-state";
import {
  MenuContent,
  MenuContextTrigger,
  MenuItem,
  MenuItemGroup,
  MenuRoot,
  MenuSeparator,
} from "@mtg-art/ui/menu";
import { ProgressBar, ProgressLabel, ProgressRoot, ProgressValueText } from "@mtg-art/ui/progress";
import { ScrollArea } from "@mtg-art/ui/scroll-area";
import { SegmentedControl } from "@mtg-art/ui/segmented-control";
import { Skeleton } from "@mtg-art/ui/skeleton";
import { Tooltip } from "@mtg-art/ui/tooltip";
import { showError, showSuccess } from "@/notify";
import {
  artSources,
  findLargerCopy,
  type JobView,
  openFoundPage,
  knownSourceName,
  saveArtFrom,
  SITE_NAMES,
  startRun,
  useRun,
  useSettings,
} from "@/store";
import { findOption } from "@/utils/options";

/** Where a saved image came from, as the table names it. */
const SOURCE_LABELS: Record<ArtSource, string> = {
  mtgpics: "MTGPics",
  scryfall: "Scryfall",
  web: "Found",
};

type Filter = "all" | "failed" | ArtSource;

/** The table's filters, each with the jobs it shows. */
const FILTERS: { value: Filter; label: string; matches: (status: JobStatus) => boolean }[] = [
  { value: "all", label: "All", matches: () => true },
  { value: "failed", label: "Failed", matches: (status) => isFailed(status) },
  ...(["scryfall", "mtgpics", "web"] as const).map((source) => ({
    value: source,
    label: SOURCE_LABELS[source],
    matches: (status: JobStatus) => hasSource(status, source),
  })),
];

/** How many placeholder rows the table shows while the run is planned. */
const PLACEHOLDER_ROWS = 6;

export function RunView() {
  const { jobs, skipped, error, summary, running, folder, listFolder } = useRun();
  const outputDir = useSettings((state) => state.settings?.outputDir);
  const [filter, setFilter] = useState<Filter>("all");

  if (jobs.length === 0 && !error && !running) {
    return (
      <EmptyState
        h="full"
        icon={<LuImageDown />}
        title="Nothing downloaded yet"
        description="Paste a card list, search Scryfall, or pick a preset or a deck, then download its art."
      />
    );
  }

  // Until the core has planned the run, the table holds placeholders the
  // shape of the rows to come.
  const starting = running && jobs.length === 0;
  const finished = jobs.filter((job) => isFinal(job.status)).length;
  const failed = jobs.filter((job) => isFailed(job.status));
  const active = FILTERS.find((option) => option.value === filter) ?? FILTERS[0];
  const shown = active ? jobs.filter((job) => active.matches(job.status)) : jobs;
  const filterItems = FILTERS.map((option) => ({
    value: option.value,
    label: `${option.label} ${jobs.filter((job) => option.matches(job.status)).length}`,
  }));

  async function copyFailed() {
    await navigator.clipboard.writeText(failed.map((job) => job.line).join("\n"));
    showSuccess(`Copied ${failed.length} failed ${failed.length === 1 ? "card" : "cards"}`);
  }

  async function openFolder() {
    const path = folder ?? outputDir;
    if (!path) return;
    try {
      await api.openFolder(path);
    } catch (cause) {
      showError("Couldn't open the folder", cause);
    }
  }

  return (
    <Flex direction="column" h="full" p="5" gap="4">
      {error && (
        <Alert status="error" title="The download couldn't start">
          {error}
        </Alert>
      )}

      {(jobs.length > 0 || starting) && (
        <Stack gap="3">
          <ProgressRoot value={starting ? null : finished} max={Math.max(jobs.length, 1)} size="sm">
            <HStack justify="space-between" mb="1">
              <ProgressLabel>
                {starting
                  ? "Getting the list ready…"
                  : running
                    ? "Downloading"
                    : summary?.cancelled
                      ? "Cancelled"
                      : "Finished"}
              </ProgressLabel>
              {!starting && (
                <ProgressValueText fontVariantNumeric="tabular-nums">
                  {finished} of {jobs.length} cards
                  {summary && !running && ` in ${formatSeconds(summary.seconds)}`}
                </ProgressValueText>
              )}
            </HStack>
            <ProgressBar />
          </ProgressRoot>

          <HStack gap="3" wrap="wrap">
            <SegmentedControl
              size="sm"
              fontVariantNumeric="tabular-nums"
              disabled={starting}
              value={filter}
              items={filterItems}
              onValueChange={(event) => {
                const chosen = findOption(FILTERS, event.value);
                if (chosen) setFilter(chosen);
              }}
            />
            <Spacer />
            {!running && failed.length > 0 && (
              <>
                <Button size="sm" variant="outline" onClick={() => void copyFailed()}>
                  <LuClipboardCopy /> Copy failed
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    void startRun({
                      kind: "list",
                      text: failed.map((job) => job.line).join("\n"),
                      folder: listFolder,
                    })
                  }
                >
                  <LuRotateCcw /> Retry failed
                </Button>
              </>
            )}
            <Button size="sm" variant="outline" onClick={() => void openFolder()}>
              <LuFolderOpen /> Open folder
            </Button>
          </HStack>
          {skipped.length > 0 && (
            <Text fontSize="xs" color="fg.muted">
              {skipped.length === 1
                ? "Skipped a line that isn't a card: "
                : `Skipped ${skipped.length} lines that aren't cards: `}
              {skipped.map((line) => line.line).join(", ")}
            </Text>
          )}
        </Stack>
      )}

      {/* Below its minimum width the table scrolls sideways rather than squeezing. */}
      <ScrollArea flex="1" minH="0" orientation="both" borderWidth="1px" rounded="md">
        <Table.Root
          size="sm"
          stickyHeader
          interactive
          tableLayout="fixed"
          minW="2xl"
          aria-busy={starting}
        >
          <Table.Header>
            <Table.Row>
              <Table.ColumnHeader w="20">
                <VisuallyHidden>Art</VisuallyHidden>
              </Table.ColumnHeader>
              <Table.ColumnHeader w="20%">Card</Table.ColumnHeader>
              <Table.ColumnHeader w="32">Status</Table.ColumnHeader>
              <Table.ColumnHeader w="28">Image</Table.ColumnHeader>
              {/* Saved takes the width left over, and its names are cut short to fit it. */}
              <Table.ColumnHeader>Saved</Table.ColumnHeader>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {starting
              ? Array.from({ length: PLACEHOLDER_ROWS }, (_, index) => (
                  <PlaceholderRow key={index} />
                ))
              : shown.map((job) => <JobRow key={job.id} job={job} />)}
          </Table.Body>
        </Table.Root>
      </ScrollArea>
    </Flex>
  );
}

/** The height of each of a row's lines, its thumbnail's: one line per image,
 * level across the columns. */
const LINE = "12";

/** What one of a row's lines shows. */
type Line =
  /** An image still to come; `active` once its job is under way. */
  | { kind: "waiting"; active: boolean }
  | { kind: "saved"; file: SavedFile }
  /** A face no art was found for. */
  | { kind: "missing"; face: string }
  /** No image: the job failed or was cancelled. */
  | { kind: "none" };

/** A row's lines: one per image it saved, or will save once it's done, so the
 * row is its final height before its art arrives. */
function rowLines(job: JobView): Line[] {
  const { status } = job;
  if (status.state === "done") {
    return [
      ...status.files.map((file): Line => ({ kind: "saved", file })),
      ...status.missing.map((face): Line => ({ kind: "missing", face })),
    ];
  }
  const line: Line =
    status.state === "failed" || status.state === "cancelled"
      ? { kind: "none" }
      : { kind: "waiting", active: status.state !== "queued" };
  return Array.from({ length: Math.max(job.images ?? 1, 1) }, () => line);
}

/** A cell's lines, each as tall as a thumbnail. */
function Lines({
  lines,
  children,
}: {
  lines: Line[];
  children: (line: Line, index: number) => React.ReactNode;
}) {
  return (
    <Stack gap="1">
      {lines.map((line, index) => (
        <Flex key={line.kind === "saved" ? line.file.path : index} h={LINE} align="center" minW="0">
          {children(line, index)}
        </Flex>
      ))}
    </Stack>
  );
}

const JobRow = memo(function JobRow({ job }: { job: JobView }) {
  const { status } = job;
  const lines = rowLines(job);
  const files = status.state === "done" ? status.files : [];

  const row = (
    <Table.Row>
      <Table.Cell>
        <Lines lines={lines}>{(line) => <Thumbnail line={line} />}</Lines>
      </Table.Cell>
      <Table.Cell>
        <Text fontWeight="medium" lineClamp={2}>
          {job.label}
        </Text>
      </Table.Cell>
      <Table.Cell>
        <StatusBadge status={status} />
      </Table.Cell>
      <Table.Cell>
        <Lines lines={lines}>{(line) => <ImageInfo line={line} />}</Lines>
      </Table.Cell>
      <Table.Cell>
        <Lines lines={lines}>
          {(line, index) =>
            // A failed job's reason, where its art would be.
            status.state === "failed" && index === 0 ? (
              <Tooltip content={status.error}>
                <Text fontSize="sm" color="fg.error" truncate>
                  {status.error}
                </Text>
              </Tooltip>
            ) : (
              <SavedLine job={job.id} line={line} />
            )
          }
        </Lines>
      </Table.Cell>
    </Table.Row>
  );
  // A row with saved art opens its menu anywhere on it.
  return files.length > 0 ? (
    <RowMenu job={job.id} files={files}>
      {row}
    </RowMenu>
  ) : (
    row
  );
});

/** A row the shape of a job's, while the run is planned. */
function PlaceholderRow() {
  return (
    <Table.Row>
      <Table.Cell>
        <Skeleton w="16" h={LINE} rounded="sm" />
      </Table.Cell>
      <Table.Cell>
        <Skeleton h="4" w="80%" />
      </Table.Cell>
      <Table.Cell>
        <Skeleton h="5" w="16" />
      </Table.Cell>
      <Table.Cell>
        <Skeleton h="4" w="20" />
      </Table.Cell>
      <Table.Cell>
        <Skeleton h="4" w="60%" />
      </Table.Cell>
    </Table.Row>
  );
}

/** Stands in for text still to come: pulsing once its job is under way, still before. */
function TextPlaceholder({ active, w }: { active: boolean; w: string }) {
  return active ? <Skeleton h="4" w={w} /> : <Skeleton h="4" w={w} variant="none" bg="bg.muted" />;
}

/** A line's thumbnail, or what stands in for it. */
function Thumbnail({ line }: { line: Line }) {
  switch (line.kind) {
    case "waiting":
      return line.active ? (
        <Skeleton w="16" h={LINE} rounded="sm" />
      ) : (
        <Skeleton w="16" h={LINE} rounded="sm" variant="none" bg="bg.muted" />
      );
    case "saved":
      return (
        <Box asChild w="16" h={LINE} rounded="sm" focusRing="outside">
          <button
            type="button"
            aria-label={`Show ${line.file.face}'s art in its folder`}
            onClick={() => void api.revealFile(line.file.path)}
          >
            <Image
              src={convertFileSrc(line.file.path)}
              alt=""
              w="full"
              maxW="none"
              h="full"
              objectFit="cover"
              rounded="sm"
              loading="lazy"
              cursor="pointer"
            />
          </button>
        </Box>
      );
    case "missing":
    case "none":
      return (
        <Center w="16" h={LINE} rounded="sm" bg="bg.subtle" color="fg.subtle" aria-hidden>
          <LuImageOff />
        </Center>
      );
  }
}

/** Where a line's image came from, over its size in pixels. */
function ImageInfo({ line }: { line: Line }) {
  switch (line.kind) {
    case "waiting":
      return <TextPlaceholder active={line.active} w="20" />;
    case "saved": {
      const { source, size } = line.file;
      return (
        <Stack gap="0" minW="0">
          <Text fontSize="xs" color="fg.muted" truncate>
            {SOURCE_LABELS[source]}
          </Text>
          <Tooltip content="Width × height, in pixels" disabled={!size}>
            <Text fontSize="sm" fontVariantNumeric="tabular-nums" whiteSpace="nowrap">
              {size ? `${size.width} × ${size.height}` : "—"}
            </Text>
          </Tooltip>
        </Stack>
      );
    }
    case "missing":
    case "none":
      return null;
  }
}

/** A line's saved file, to show in its folder, with what else can be done with it. */
function SavedLine({ job, line }: { job: number; line: Line }) {
  switch (line.kind) {
    case "waiting":
      return <TextPlaceholder active={line.active} w="60%" />;
    case "saved": {
      const { file } = line;
      return (
        <HStack flex="1" minW="0" gap="1">
          <Tooltip
            content={
              <>
                <Text wordBreak="break-all">{file.path}</Text>
                <Text mt="1">
                  Click to show it in its folder, or right-click the row for its other sources
                </Text>
              </>
            }
            contentProps={{ maxW: "md" }}
          >
            <Box
              asChild
              flex="1"
              minW="0"
              fontSize="sm"
              truncate
              cursor="pointer"
              textAlign="start"
              rounded="sm"
              focusRing="outside"
            >
              <button type="button" onClick={() => void api.revealFile(file.path)}>
                {file.face} · {file.artist}
              </button>
            </Box>
          </Tooltip>
          <FileAction job={job} file={file} />
        </HStack>
      );
    }
    case "missing":
      return (
        <Text fontSize="sm" color="fg.warning" truncate>
          No art found for {line.face}
        </Text>
      );
    case "none":
      return null;
  }
}

/** What a choice in a row's menu does. */
type RowAction =
  | { kind: "reveal"; file: SavedFile }
  | { kind: "use"; file: SavedFile; known: KnownSource };

/** A row's right-click menu: for each saved image, the places its art is known
 * to be, to save it again from one, and a way to show it in its folder. */
function RowMenu({
  job,
  files,
  children,
}: {
  job: number;
  files: SavedFile[];
  children: React.ReactNode;
}) {
  // Per file, where its art is known to be; `null` while they're asked for.
  const [known, setKnown] = useState<KnownSource[][] | null>(null);
  const actions = new Map<string, RowAction>();

  async function load() {
    setKnown(null);
    setKnown(await Promise.all(files.map((file) => artSources(job, file.faceIndex))));
  }

  const groups = files.map((file, position) => {
    const reveal = `${file.faceIndex}:reveal`;
    actions.set(reveal, { kind: "reveal", file });
    const sources = known?.[position];
    return (
      <MenuItemGroup
        key={file.faceIndex}
        title={files.length > 1 ? `${file.face}: use the art from` : "Use the art from"}
      >
        {sources === undefined ? (
          <MenuItem value={`${file.faceIndex}:loading`} disabled>
            <Spinner size="xs" /> Looking…
          </MenuItem>
        ) : (
          sources.map((source, index) => {
            const value = `${file.faceIndex}:${index}`;
            actions.set(value, { kind: "use", file, known: source });
            return (
              <MenuItem key={value} value={value} disabled={source.current}>
                <Stack gap="0">
                  <Text>
                    {knownSourceName(source)}
                    {source.size && ` · ${source.size.width} × ${source.size.height}`}
                    {source.current && " (saved now)"}
                  </Text>
                  {source.site && (
                    <Text fontSize="xs" color="fg.muted">
                      Found from {SITE_NAMES[source.site]}
                    </Text>
                  )}
                </Stack>
              </MenuItem>
            );
          })
        )}
        <MenuItem value={reveal}>Show in folder</MenuItem>
      </MenuItemGroup>
    );
  });

  return (
    <MenuRoot
      onOpenChange={(event) => {
        if (event.open) void load();
      }}
      onSelect={(event) => {
        const action = actions.get(event.value);
        if (action?.kind === "reveal") void api.revealFile(action.file.path);
        if (action?.kind === "use") void saveArtFrom(job, action.file.faceIndex, action.known);
      }}
    >
      <MenuContextTrigger asChild>{children}</MenuContextTrigger>
      <MenuContent>
        {groups.flatMap((group, index) =>
          index === 0 ? [group] : [<MenuSeparator key={`separator-${index}`} />, group],
        )}
      </MenuContent>
    </MenuRoot>
  );
}

/** The places a larger copy of Scryfall's art can be looked for, by logo. */
const SITES: { site: Site; icon: IconType; tip: (file: SavedFile) => string }[] = [
  { site: "lens", icon: SiGooglelens, tip: () => "Search Google Lens for this art" },
  {
    site: "artstation",
    icon: SiArtstation,
    tip: (file) => `Look for ${file.artist} on ArtStation`,
  },
  {
    site: "deviantart",
    icon: SiDeviantart,
    tip: (file) => `Look for ${file.artist} on DeviantArt`,
  },
  { site: "pixiv", icon: SiPixiv, tip: (file) => `Look for ${file.artist} on Pixiv` },
  { site: "instagram", icon: SiInstagram, tip: (file) => `Look for ${file.artist} on Instagram` },
  { site: "website", icon: LuGlobe, tip: (file) => `Open ${file.artist}'s website` },
];

/** What can be done with a saved image beyond showing it: look for a larger
 * copy of Scryfall's art, or open the page a found image came from. */
function FileAction({ job, file }: { job: number; file: SavedFile }) {
  const searchMtgpicsArt = useSettings((state) => state.settings?.searchMtgpicsArt ?? false);
  if (file.source === "scryfall" || (file.source === "mtgpics" && searchMtgpicsArt)) {
    return (
      <HStack gap="0">
        {SITES.map(({ site, icon: Icon, tip }) => (
          <Tooltip key={site} content={tip(file)}>
            <IconButton
              size="2xs"
              variant="ghost"
              aria-label={`${tip(file)}, for ${file.face}`}
              onClick={() => void findLargerCopy(job, file.faceIndex, site)}
            >
              {/* Simple Icons' logos are role="img"; the button's label names them. */}
              <Icon aria-hidden />
            </IconButton>
          </Tooltip>
        ))}
      </HStack>
    );
  }
  if (file.source === "web" && file.page) {
    return (
      <Tooltip content="Open the page it came from">
        <IconButton
          size="2xs"
          variant="ghost"
          aria-label={`Open the page ${file.face}'s art came from`}
          onClick={() => void openFoundPage(job, file.faceIndex)}
        >
          <LuExternalLink />
        </IconButton>
      </Tooltip>
    );
  }
  return null;
}

/** How each status is shown: its badge colour and words, and whether it's under way. */
function statusLook(status: JobStatus): { palette: string; label: string; busy?: boolean } {
  switch (status.state) {
    case "queued":
      return { palette: "gray", label: "Queued" };
    case "resolving":
      return { palette: "blue", label: "Looking up", busy: true };
    case "downloading":
      return { palette: "blue", label: "Downloading", busy: true };
    case "done":
      return status.missing.length > 0
        ? { palette: "orange", label: "Partly done" }
        : { palette: "green", label: "Done" };
    case "failed":
      return { palette: "red", label: "Failed" };
    case "cancelled":
      return { palette: "gray", label: "Cancelled" };
  }
}

function StatusBadge({ status }: { status: JobStatus }) {
  const { palette, label, busy } = statusLook(status);
  return (
    <Badge colorPalette={palette}>
      {busy && <Spinner size="xs" />} {label}
    </Badge>
  );
}

function isFinal(status: JobStatus): boolean {
  return status.state === "done" || status.state === "failed" || status.state === "cancelled";
}

/** Failed outright, or done with some faces missing. */
function isFailed(status: JobStatus): boolean {
  return status.state === "failed" || (status.state === "done" && status.missing.length > 0);
}

function hasSource(status: JobStatus, source: ArtSource): boolean {
  return status.state === "done" && status.files.some((file) => file.source === source);
}

function formatSeconds(seconds: number): string {
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
}
