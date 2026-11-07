import {
  Alert,
  Badge,
  Box,
  Button,
  EmptyState,
  Flex,
  HStack,
  Image,
  Progress,
  SegmentGroup,
  Spinner,
  Stack,
  Table,
  Text,
  VStack,
} from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { memo, useState } from "react";
import { LuClipboardCopy, LuFolderOpen, LuImageDown, LuRotateCcw } from "react-icons/lu";

import { api, errorText } from "@/api";
import type { ArtSource } from "@/bindings/ArtSource";
import type { JobStatus } from "@/bindings/JobStatus";
import { toaster } from "@/components/ui/toaster";
import { type JobView, startRun, useRun, useSettings } from "@/store";

const FILTERS = ["All", "Failed", "Scryfall", "MTGPics"] as const;
type Filter = (typeof FILTERS)[number];

function isFilter(value: string | null): value is Filter {
  return (FILTERS as readonly (string | null)[]).includes(value);
}

export function RunView() {
  const { jobs, skipped, error, summary, running } = useRun();
  const outputDir = useSettings((state) => state.settings?.outputDir);
  const [filter, setFilter] = useState<Filter>("All");

  if (jobs.length === 0 && !error) {
    return (
      <EmptyState.Root h="full">
        <EmptyState.Content>
          <EmptyState.Indicator>{running ? <Spinner /> : <LuImageDown />}</EmptyState.Indicator>
          <VStack textAlign="center">
            <EmptyState.Title>
              {running ? "Getting the list ready…" : "Nothing downloaded yet"}
            </EmptyState.Title>
            <EmptyState.Description>
              {running
                ? "Looking up the cards to download."
                : "Paste a card list, search Scryfall, or pick a preset, then download its art."}
            </EmptyState.Description>
          </VStack>
        </EmptyState.Content>
      </EmptyState.Root>
    );
  }

  const finished = jobs.filter((job) => isFinal(job.status)).length;
  const failed = jobs.filter((job) => job.status.state === "failed" || isPartial(job.status));
  const fromScryfall = jobs.filter((job) => hasSource(job.status, "scryfall")).length;
  const fromMtgpics = jobs.filter((job) => hasSource(job.status, "mtgpics")).length;
  const shown = jobs.filter((job) => matchesFilter(job, filter));

  async function copyFailed() {
    await navigator.clipboard.writeText(failed.map((job) => job.line).join("\n"));
    toaster.create({ type: "success", title: `Copied ${failed.length} failed cards` });
  }

  async function openFolder() {
    if (!outputDir) return;
    try {
      await api.openFolder(outputDir);
    } catch (e) {
      toaster.create({
        type: "error",
        title: "Couldn't open the folder",
        description: errorText(e),
      });
    }
  }

  return (
    <Flex direction="column" h="full" p="5" gap="4">
      {error && (
        <Alert.Root status="error">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>The download couldn&apos;t start</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert.Root>
      )}

      {jobs.length > 0 && (
        <Stack gap="3">
          <Progress.Root value={finished} max={jobs.length} size="sm" colorPalette="blue">
            <HStack justify="space-between" mb="1">
              <Progress.Label>
                {running ? "Downloading" : summary?.cancelled ? "Cancelled" : "Finished"}
              </Progress.Label>
              <Text fontSize="sm" color="fg.muted">
                {finished} of {jobs.length} cards
                {summary && !running && ` in ${formatSeconds(summary.seconds)}`}
              </Text>
            </HStack>
            <Progress.Track>
              <Progress.Range />
            </Progress.Track>
          </Progress.Root>

          <HStack gap="3" wrap="wrap">
            <SegmentGroup.Root
              size="sm"
              value={filter}
              onValueChange={(event) => isFilter(event.value) && setFilter(event.value)}
            >
              <SegmentGroup.Indicator />
              <SegmentGroup.Items
                items={[
                  { value: "All", label: `All ${jobs.length}` },
                  { value: "Failed", label: `Failed ${failed.length}` },
                  { value: "Scryfall", label: `Scryfall ${fromScryfall}` },
                  { value: "MTGPics", label: `MTGPics ${fromMtgpics}` },
                ]}
              />
            </SegmentGroup.Root>
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
                    void startRun({ kind: "list", text: failed.map((job) => job.line).join("\n") })
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
              <Table.ColumnHeader w="20" />
              <Table.ColumnHeader>Card</Table.ColumnHeader>
              <Table.ColumnHeader w="32">Status</Table.ColumnHeader>
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
        <Stack gap="1">
          {files.map((file) => (
            <HStack
              key={file.path}
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
          ))}
        </Stack>
      </Table.Cell>
    </Table.Row>
  );
});

function StatusBadge({ status }: { status: JobStatus }) {
  switch (status.state) {
    case "queued":
      return <Badge>Queued</Badge>;
    case "resolving":
      return (
        <Badge colorPalette="blue">
          <Spinner size="xs" /> Looking up
        </Badge>
      );
    case "downloading":
      return (
        <Badge colorPalette="blue">
          <Spinner size="xs" /> Downloading
        </Badge>
      );
    case "done":
      return status.missing.length > 0 ? (
        <Badge colorPalette="orange">Partly done</Badge>
      ) : (
        <Badge colorPalette="green">Done</Badge>
      );
    case "failed":
      return <Badge colorPalette="red">Failed</Badge>;
    case "cancelled":
      return <Badge>Cancelled</Badge>;
  }
}

function SourceBadge({ source }: { source: ArtSource }) {
  return source === "mtgpics" ? (
    <Badge colorPalette="purple" variant="subtle">
      MTGPics
    </Badge>
  ) : (
    <Badge colorPalette="teal" variant="subtle">
      Scryfall
    </Badge>
  );
}

function isFinal(status: JobStatus): boolean {
  return status.state === "done" || status.state === "failed" || status.state === "cancelled";
}

function isPartial(status: JobStatus): boolean {
  return status.state === "done" && status.missing.length > 0;
}

function hasSource(status: JobStatus, source: ArtSource): boolean {
  return status.state === "done" && status.files.some((file) => file.source === source);
}

function matchesFilter(job: JobView, filter: Filter): boolean {
  switch (filter) {
    case "All":
      return true;
    case "Failed":
      return job.status.state === "failed" || isPartial(job.status);
    case "Scryfall":
      return hasSource(job.status, "scryfall");
    case "MTGPics":
      return hasSource(job.status, "mtgpics");
  }
}

function formatSeconds(seconds: number): string {
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
}
