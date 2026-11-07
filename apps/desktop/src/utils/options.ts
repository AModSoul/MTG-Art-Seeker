/** One choice in a select, segmented control or tab list, for a union of strings. */
export interface Option<T extends string> {
  value: T;
  label: string;
}

/** The option a component reported choosing, narrowed back to its type — or
 * `undefined` for a value that isn't one of them. */
export function findOption<T extends string>(
  options: readonly Option<T>[],
  value: string | null | undefined,
): T | undefined {
  return options.find((option) => option.value === value)?.value;
}
