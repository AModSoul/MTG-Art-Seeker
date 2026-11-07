/**
 * The document title the matched routes ask for, resolved the way TanStack Router's head pipeline
 * does: the deepest route first, and within a route's `meta` the last entry first.
 */
export function documentTitleFromMatches(
  matches: Array<{ meta?: Array<{ title?: string } | undefined> | undefined }>,
): string | undefined {
  for (const match of matches.toReversed()) {
    for (const meta of (match.meta ?? []).toReversed()) {
      if (meta?.title) return meta.title;
    }
  }
  return undefined;
}
