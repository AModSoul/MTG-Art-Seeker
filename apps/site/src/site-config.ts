/**
 * Everything about where the site lives and what it links to.
 *
 * With `CUSTOM_DOMAIN` set, the site is served from the domain's root; set to `null`, from GitHub
 * Pages' project URL, under the repository's name. The domain itself is configured in the
 * repository's Pages settings and the domain's DNS: a deploy from Actions ignores `CNAME` files.
 */
export const CUSTOM_DOMAIN: string | null = "getmtg.art";

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
