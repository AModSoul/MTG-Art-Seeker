/**
 * Everything about where the site lives and what it links to.
 *
 * Until `CUSTOM_DOMAIN` is set, the site is served from GitHub Pages' project URL, under the
 * repository's name. Setting it serves the site from the domain's root and writes a `CNAME` file
 * into the build; the domain's DNS and the repository's Pages settings have to point at it too.
 */
export const CUSTOM_DOMAIN: string | null = null;

/** The repository whose Releases ship the desktop app. */
export const GITHUB_OWNER = "AModSoul";
export const GITHUB_REPO = "mtg-art-downloader-rust";

export const GITHUB_REPO_URL = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}`;
export const GITHUB_RELEASES_URL = `${GITHUB_REPO_URL}/releases`;

export const DISCORD_INVITE_URL = "https://discord.gg/A5AkkyP8CU";

/** The path the site is served under, with leading and trailing slashes. */
export const BASE_PATH = CUSTOM_DOMAIN ? "/" : `/${GITHUB_REPO}/`;

/** Where the site's root is, for absolute links in `<meta>` tags. */
export const SITE_URL = CUSTOM_DOMAIN
  ? `https://${CUSTOM_DOMAIN}/`
  : `https://${GITHUB_OWNER.toLowerCase()}.github.io/${GITHUB_REPO}/`;

export const APP_NAME = "MTG Art Downloader";

export const DESCRIPTION =
  "Download Magic: The Gathering card art in bulk from a deck list, a Scryfall search or an EDHREC list. MTGPics' larger, uncropped art where it has it, Scryfall's otherwise.";
