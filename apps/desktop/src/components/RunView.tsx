import {
  Badge,
  Box,
  Button,
  Flex,
  HStack,
  IconButton,
  Image,
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
import { ProgressBar, ProgressLabel, ProgressRoot } from "@mtg-art/ui/progress";
import { SegmentedControl } from "@mtg-art/ui/segmented-control";
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

type Filter = "all" | "failed" | "scryfall" | "mtgpics" | "web";

/** The table's filters, each with the jobs it shows. */
const FILTERS: { value: Filter; label: string; matches: (status: JobStatus) => boolean }[] = [
  { value: "all", label: "All", matches: () => true },
  { value: "failed", label: "Failed", matches: (status) => isFailed(status) },
  { value: "scryfall", label: "Scryfall", matches: (status) => hasSource(status, "scryfall") },
  { value: "mtgpics", label: "MTGPics", matches: (status) => hasSource(status, "mtgpics") },
  { value: "web", label: "Found", matches: (status) => hasSource(status, "web") },
];

export function RunView() {
  const { jobs, skipped, error, summary, running, folder, listFolder } = useRun();
  const outputDir = useSettings((state) => state.settings?.outputDir);
  const [filter, setFilter] = useState<Filter>("all");

  if (jobs.length === 0 && !error) {
    return (
      <EmptyState
        h="full"
        icon={running ? <Spinner /> : <LuImageDown />}
        title={running ? "Getting the list ready…" : "Nothing downloaded yet"}
        description={
          running
            ? "Looking up the cards to download."
            : "Paste a card list, search Scryfall, or pick a preset or a deck, then download its art."
        }
      />
    );
  }

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

      {jobs.length > 0 && (
        <Stack gap="3">
          <ProgressRoot value={finished} max={jobs.length} size="sm" colorPalette="emerald">
            <HStack justify="space-between" mb="1">
              <ProgressLabel>
                {running ? "Downloading" : summary?.cancelled ? "Cancelled" : "Finished"}
              </ProgressLabel>
              <Text fontSize="sm" color="fg.muted">
                {finished} of {jobs.length} cards
                {summary && !running && ` in ${formatSeconds(summary.seconds)}`}
              </Text>
            </HStack>
            <ProgressBar />
          </ProgressRoot>

          <HStack gap="3" wrap="wrap">
            <SegmentedControl
              size="sm"
              value={filter}
              items={filterItems}
              onValueChange={(event) => {
                const chosen = findOption(FILTERS, event.value);
                if (chosen) setFilter(chosen);
              }}
            />
            <Box flex="1" />
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

      <Box flex="1" minH="0" overflowY="auto" borderWidth="1px" rounded="md">
        <Table.Root size="sm" stickyHeader interactive>
          <Table.Header>
            <Table.Row>
              <Table.ColumnHeader w="20">
                <VisuallyHidden>Art</VisuallyHidden>
              </Table.ColumnHeader>
              <Table.ColumnHeader>Card</Table.ColumnHeader>
              <Table.ColumnHeader w="32">Status</Table.ColumnHeader>
              <Table.ColumnHeader w="28">Resolution</Table.ColumnHeader>
              {/* Saved takes the width left over, and its names are cut short to fit it. */}
              <Table.ColumnHeader w="full" maxW="0">
                Saved
              </Table.ColumnHeader>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {shown.map((job) => (
              <JobRow key={job.id} job={job} />
            ))}
          </Table.Body>
        </Table.Root>
      </Box>
    </Flex>
  );
}

/** The height of a saved image's line, the same under Resolution and Saved. */
const FILE_LINE = "6";

/** A face's line on a card with more than one image: twice as tall, with room
 * for the face's own thumbnail beside it. */
const FACE_LINE = "12";

const JobRow = memo(function JobRow({ job }: { job: JobView }) {
  const { status } = job;
  const files = status.state === "done" ? status.files : [];
  const line = files.length > 1 ? FACE_LINE : FILE_LINE;

  const row = (
    <Table.Row>
      <Table.Cell>
        {/* A thumbnail per face, level with its lines; one card shows its only one. */}
        <Stack gap="1">
          {files.map((file) => (
            <Box key={file.path} asChild w="16" h="12" rounded="sm" focusRing="outside">
              <button
                type="button"
                aria-label={`Show ${file.face}'s art in its folder`}
                onClick={() => void api.revealFile(file.path)}
              >
                <Image
                  src={convertFileSrc(file.path)}
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
          ))}
        </Stack>
      </Table.Cell>
      <Table.Cell>
        <Text fontWeight="medium">{job.label}</Text>
        {status.state === "failed" && (
          <Text fontSize="xs" color="fg.error">
            {status.error}
          </Text>
        )}
        {status.state === "done" && status.missing.length > 0 && (
          <Text fontSize="xs" color="fg.warning">
            No art found for {status.missing.join(", ")}
          </Text>
        )}
      </Table.Cell>
      <Table.Cell>
        <StatusBadge status={status} />
      </Table.Cell>
      <Table.Cell>
        {/* One line per saved image, level with its line under Saved. */}
        <Stack gap="1">
          {files.map((file) => (
            <Flex key={file.path} h={line} align="center">
              <Tooltip content="Width × height, in pixels" disabled={!file.size}>
                <Text
                  fontSize="sm"
                  color="fg.muted"
                  fontVariantNumeric="tabular-nums"
                  whiteSpace="nowrap"
                >
                  {file.size ? `${file.size.width} × ${file.size.height}` : "—"}
                </Text>
              </Tooltip>
            </Flex>
          ))}
        </Stack>
      </Table.Cell>
      <Table.Cell w="full" maxW="0">
        <Stack gap="1">
          {files.map((file) => (
            <HStack key={file.path} h={line} gap="1">
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
                <HStack
                  asChild
                  flex="1"
                  minW="0"
                  gap="2"
                  cursor="pointer"
                  textAlign="start"
                  rounded="sm"
                  focusRing="outside"
                >
                  <button type="button" onClick={() => void api.revealFile(file.path)}>
                    <SourceBadge source={file.source} />
                    <Text fontSize="sm" truncate>
                      {file.face} · {file.artist}
                    </Text>
                  </button>
                </HStack>
              </Tooltip>
              <FileAction job={job.id} file={file} />
            </HStack>
          ))}
        </Stack>
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

const SOURCE_LOOK: Record<ArtSource, { palette: string; label: string }> = {
  mtgpics: { palette: "purple", label: "MTGPics" },
  scryfall: { palette: "teal", label: "Scryfall" },
  web: { palette: "blue", label: "Found" },
};

function SourceBadge({ source }: { source: ArtSource }) {
  const { palette, label } = SOURCE_LOOK[source];
  return (
    <Badge colorPalette={palette} variant="subtle">
      {label}
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
