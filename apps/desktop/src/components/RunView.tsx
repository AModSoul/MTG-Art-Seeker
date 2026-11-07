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
import type { SavedFile } from "@/bindings/SavedFile";
import type { Site } from "@/bindings/Site";
import { Alert } from "@mtg-art/ui/alert";
import { EmptyState } from "@mtg-art/ui/empty-state";
import { ProgressBar, ProgressLabel, ProgressRoot } from "@mtg-art/ui/progress";
import { SegmentedControl } from "@mtg-art/ui/segmented-control";
import { Tooltip } from "@mtg-art/ui/tooltip";
import { showError, showSuccess } from "@/notify";
import {
  findLargerCopy,
  type JobView,
  openFoundPage,
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
            : "Paste a card list, search Scryfall, or pick a preset, then download its art."
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
          <ProgressRoot value={finished} max={jobs.length} size="sm" colorPalette="blue">
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
              <Table.ColumnHeader>Saved</Table.ColumnHeader>
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

const JobRow = memo(function JobRow({ job }: { job: JobView }) {
  const { status } = job;
  const files = status.state === "done" ? status.files : [];
  const first = files[0];

  return (
    <Table.Row>
      <Table.Cell>
        {first && (
          <Image
            src={convertFileSrc(first.path)}
            alt=""
            w="16"
            h="12"
            objectFit="cover"
            rounded="sm"
            loading="lazy"
            cursor="pointer"
            onClick={() => void api.revealFile(first.path)}
          />
        )}
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
            <Flex key={file.path} h={FILE_LINE} align="center" title="Width × height, in pixels">
              <Text fontSize="sm" color="fg.muted" fontVariantNumeric="tabular-nums">
                {file.size ? `${file.size.width} × ${file.size.height}` : "—"}
              </Text>
            </Flex>
          ))}
        </Stack>
      </Table.Cell>
      <Table.Cell>
        <Stack gap="1">
          {files.map((file) => (
            <HStack key={file.path} h={FILE_LINE} gap="1">
              <HStack
                flex="1"
                minW="0"
                gap="2"
                cursor="pointer"
                title={`${file.path}\nClick to show it in its folder`}
                onClick={() => void api.revealFile(file.path)}
              >
                <SourceBadge source={file.source} />
                <Text fontSize="sm" truncate>
                  {file.face} · {file.artist}
                </Text>
              </HStack>
              <FileAction job={job.id} file={file} />
            </HStack>
          ))}
        </Stack>
      </Table.Cell>
    </Table.Row>
  );
});

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
  if (file.source === "scryfall") {
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
              <Icon />
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
