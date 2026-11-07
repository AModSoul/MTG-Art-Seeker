import {
  Box,
  Button,
  Code,
  Heading,
  HStack,
  Link,
  List,
  Separator,
  Stack,
  Tabs,
  Text,
} from "@chakra-ui/react";
import { Alert } from "@mtg-art/ui/alert";
import { useState, type ReactNode } from "react";
import { LuDownload } from "react-icons/lu";

import {
  type Arch,
  type Artifact,
  archLabel,
  classify,
  formatSize,
  type Os,
  OS_LABEL,
  OSES,
  recommend,
} from "@/downloads/artifacts";
import type { Release, ReleaseAsset } from "@/downloads/github";
import { GITHUB_RELEASES_URL } from "@/site-config";

export type ReleaseState =
  | { status: "loading" }
  | { status: "ready"; release: Release }
  | { status: "error"; message: string };

export type DownloadPanelProps = {
  state: ReleaseState;
  /** The visitor's system, when the browser says. */
  os: Os | null;
  /** The visitor's processor, when the browser says. */
  arch: Arch | null;
};

/** The latest release's downloads, sorted by system, with the visitor's recommended first. */
export function DownloadPanel({ state, os, arch }: DownloadPanelProps) {
  switch (state.status) {
    case "loading":
      return (
        <Stack gap={3} align="flex-start">
          <Button colorPalette="blue" size="lg" disabled>
            <LuDownload /> Finding the latest release…
          </Button>
          <ReleasesLink />
        </Stack>
      );
    case "error":
      return (
        <Stack gap={4} align="flex-start">
          <Alert status="warning" title="Couldn't load the latest release from GitHub">
            {state.message} The installers are all on GitHub's releases page.
          </Alert>
          <Button asChild colorPalette="blue" size="lg">
            <a href={`${GITHUB_RELEASES_URL}/latest`}>Go to the latest release</a>
          </Button>
        </Stack>
      );
    case "ready":
      return <ReleaseDownloads release={state.release} os={os} arch={arch} />;
  }
}

function ReleaseDownloads({
  release,
  os,
  arch,
}: {
  release: Release;
  os: Os | null;
  arch: Arch | null;
}) {
  const { apps, cli, other } = classify(release.assets);
  const tabs = OSES.filter((candidate) => apps[candidate].length > 0);
  const [picked, setPicked] = useState<Os | null>(null);
  const tab = picked ?? (os && tabs.includes(os) ? os : tabs[0]);

  return (
    <Stack gap={8}>
      <ReleaseLine release={release} />

      {tabs.length === 0 && cli.length === 0 && other.length === 0 ? (
        <Stack gap={4} align="flex-start">
          <Text>This release has no downloads yet.</Text>
          <ReleasesLink />
        </Stack>
      ) : null}

      {tab ? (
        <Tabs.Root
          value={tab}
          onValueChange={(details) => setPicked(OSES.find((o) => o === details.value) ?? null)}
          colorPalette="blue"
        >
          <Tabs.List>
            {tabs.map((candidate) => (
              <Tabs.Trigger key={candidate} value={candidate}>
                {OS_LABEL[candidate]}
              </Tabs.Trigger>
            ))}
          </Tabs.List>
          {tabs.map((candidate) => (
            <Tabs.Content key={candidate} value={candidate} pt={6}>
              <OsDownloads
                os={candidate}
                artifacts={apps[candidate]}
                arch={candidate === os ? arch : null}
              />
            </Tabs.Content>
          ))}
        </Tabs.Root>
      ) : null}

      {cli.length > 0 ? (
        <Section title="Command line tool">
          <Text color="fg.muted">
            <Code>mtg-art</Code> does what the app does, from a terminal, and reads the same
            settings.
          </Text>
          <FileList
            files={cli.map((artifact) => ({
              asset: artifact.asset,
              label: [artifact.os && OS_LABEL[artifact.os], archLabel(artifact.os, artifact.arch)]
                .filter(Boolean)
                .join(", "),
            }))}
          />
        </Section>
      ) : null}

      {other.length > 0 ? (
        <Section title="Other files">
          <FileList files={other.map((asset) => ({ asset, label: asset.name }))} />
        </Section>
      ) : null}
    </Stack>
  );
}

function ReleaseLine({ release }: { release: Release }) {
  const version = release.tag_name.replace(/^v/, "");
  const date = release.published_at
    ? new Intl.DateTimeFormat("en", { dateStyle: "long", timeZone: "UTC" }).format(
        new Date(release.published_at),
      )
    : null;
  return (
    <Text color="fg.muted">
      Version {version}
      {date ? `, released ${date}` : ""}.{" "}
      <Link href={release.html_url} target="_blank" rel="noreferrer">
        Release notes
      </Link>
    </Text>
  );
}

function OsDownloads({
  os,
  artifacts,
  arch,
}: {
  os: Os;
  artifacts: Artifact[];
  arch: Arch | null;
}) {
  const recommended = recommend(artifacts, os, arch);
  const rest = artifacts.filter((artifact) => artifact !== recommended);
  const help = INSTALL_HELP[os](artifacts);

  return (
    <Stack gap={8}>
      {recommended ? (
        <Stack gap={2} align="flex-start">
          <Button asChild colorPalette="blue" size="xl">
            <a href={recommended.asset.browser_download_url}>
              <LuDownload /> Download for {OS_LABEL[os]}
            </a>
          </Button>
          <Text fontSize="sm" color="fg.muted">
            {describe(recommended)}
          </Text>
        </Stack>
      ) : null}

      {rest.length > 0 ? (
        <Section title="Other downloads" level={3}>
          <FileList
            files={rest.map((artifact) => ({
              asset: artifact.asset,
              label: [artifact.rule.label, archLabel(os, artifact.arch)].filter(Boolean).join(", "),
              note: artifact.rule.note,
            }))}
          />
        </Section>
      ) : null}

      {help ? (
        <Alert status="info" title={help.title}>
          {help.body}
        </Alert>
      ) : null}
    </Stack>
  );
}

const describe = (artifact: Artifact) =>
  [artifact.rule.label, archLabel(artifact.os, artifact.arch), formatSize(artifact.asset.size)]
    .filter(Boolean)
    .join(" · ");

function Section({
  title,
  level = 2,
  children,
}: {
  title: string;
  level?: 2 | 3;
  children: ReactNode;
}) {
  return (
    <Stack gap={3}>
      <Heading as={level === 2 ? "h2" : "h3"} size={level === 2 ? "lg" : "md"}>
        {title}
      </Heading>
      {children}
    </Stack>
  );
}

function FileList({ files }: { files: { asset: ReleaseAsset; label: string; note?: string }[] }) {
  return (
    <Stack as="ul" listStyleType="none" gap={0} borderWidth="1px" rounded="lg" overflow="hidden">
      {files.map(({ asset, label, note }, index) => (
        <Box as="li" key={asset.name}>
          {index > 0 ? <Separator /> : null}
          <HStack justify="space-between" align="flex-start" gap={4} px={4} py={3}>
            <Stack gap={0} minW={0}>
              <Link href={asset.browser_download_url} fontWeight="medium" wordBreak="break-all">
                {label || asset.name}
              </Link>
              {note ? (
                <Text fontSize="sm" color="fg.muted">
                  {note}
                </Text>
              ) : null}
            </Stack>
            <Text fontSize="sm" color="fg.muted" flexShrink={0}>
              {formatSize(asset.size)}
            </Text>
          </HStack>
        </Box>
      ))}
    </Stack>
  );
}

function ReleasesLink() {
  return (
    <Link href={GITHUB_RELEASES_URL} target="_blank" rel="noreferrer" fontSize="sm">
      All releases on GitHub
    </Link>
  );
}

/** What to expect the first time the app is opened, for the files a system's tab offers. */
const INSTALL_HELP: Record<
  Os,
  (artifacts: Artifact[]) => { title: string; body: ReactNode } | null
> = {
  windows: () => ({
    title: "Windows may warn you the first time",
    body: (
      <>
        The installer isn't signed yet, so SmartScreen may say it protected your PC. Click{" "}
        <strong>More info</strong>, then <strong>Run anyway</strong>.
      </>
    ),
  }),
  macos: () => ({
    title: "macOS asks you to allow the app once",
    body: (
      <List.Root as="ol" ps={5} gap={1}>
        <List.Item>Open the disk image and drag the app into Applications.</List.Item>
        <List.Item>
          Open it. macOS says it can't check the app; click <strong>Done</strong>, not Move to
          Trash.
        </List.Item>
        <List.Item>
          In <strong>System Settings → Privacy &amp; Security</strong>, click{" "}
          <strong>Open Anyway</strong> next to the message about the app, and confirm.
        </List.Item>
      </List.Root>
    ),
  }),
  linux: (artifacts) =>
    artifacts.some((artifact) => artifact.rule.id === "appimage")
      ? {
          title: "Running the AppImage",
          body: (
            <>
              Make it executable, then run it: <Code>chmod +x *.AppImage</Code>. The packages
              install with your system's package manager instead.
            </>
          ),
        }
      : null,
};
