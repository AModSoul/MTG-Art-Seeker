import { ScrollArea as ChakraScrollArea, useScrollAreaContext } from "@chakra-ui/react";

export interface ScrollAreaProps extends ChakraScrollArea.RootProps {
  /** Which way the content scrolls. */
  orientation?: "vertical" | "horizontal" | "both";
  /** Names the scrolling region for assistive technology, while it scrolls. */
  label?: string;
  viewportProps?: ChakraScrollArea.ViewportProps;
  contentProps?: ChakraScrollArea.ContentProps;
  ref?: React.Ref<HTMLDivElement>;
  /** The element that scrolls. */
  viewportRef?: React.Ref<HTMLDivElement>;
}

/**
 * Scrolls its children with the same thin scrollbar on every system, instead of the OS's own.
 * Size the root as you would the box that scrolls: `flex="1" minH="0"` in a flex column, or a
 * `height`/`maxH`.
 */
export function ScrollArea({
  ref,
  viewportRef,
  orientation = "vertical",
  label,
  viewportProps,
  contentProps,
  children,
  ...rest
}: ScrollAreaProps) {
  return (
    <ChakraScrollArea.Root ref={ref} {...rest}>
      <Viewport ref={viewportRef} orientation={orientation} label={label} {...viewportProps}>
        <ChakraScrollArea.Content {...contentProps}>{children}</ChakraScrollArea.Content>
      </Viewport>
      {orientation !== "horizontal" && <ChakraScrollArea.Scrollbar orientation="vertical" />}
      {orientation !== "vertical" && <ChakraScrollArea.Scrollbar orientation="horizontal" />}
      {orientation === "both" && <ChakraScrollArea.Corner />}
    </ChakraScrollArea.Root>
  );
}

/**
 * The element that scrolls. While its content overflows the way it scrolls, it takes focus,
 * so the arrow keys can scroll it even when nothing in it is focusable; WebKit, which the
 * desktop app runs in on macOS and Linux, doesn't do that by itself. Then it's a region if it
 * has a `label`, and a group otherwise, rather than the presentational element it is when
 * there's nothing to scroll. The other axis is ignored: a fraction of a pixel's rounding can
 * make it overflow.
 */
function Viewport({
  ref,
  orientation,
  label,
  ...props
}: ChakraScrollArea.ViewportProps & {
  orientation: NonNullable<ScrollAreaProps["orientation"]>;
  label?: string;
  ref?: React.Ref<HTMLDivElement>;
}) {
  const { hasOverflowX, hasOverflowY } = useScrollAreaContext();
  const scrolls =
    (orientation !== "vertical" && hasOverflowX) || (orientation !== "horizontal" && hasOverflowY);
  return (
    <ChakraScrollArea.Viewport
      ref={ref}
      focusRing="inside"
      {...(scrolls && { tabIndex: 0, role: label ? "region" : "group", "aria-label": label })}
      {...props}
    />
  );
}
