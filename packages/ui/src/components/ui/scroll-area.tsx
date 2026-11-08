import { ScrollArea as ChakraScrollArea } from "@chakra-ui/react";

export interface ScrollAreaProps extends ChakraScrollArea.RootProps {
  /** Which way the content scrolls. */
  orientation?: "vertical" | "horizontal" | "both";
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
  viewportProps,
  contentProps,
  children,
  ...rest
}: ScrollAreaProps) {
  return (
    <ChakraScrollArea.Root ref={ref} {...rest}>
      <ChakraScrollArea.Viewport ref={viewportRef} {...viewportProps}>
        <ChakraScrollArea.Content {...contentProps}>{children}</ChakraScrollArea.Content>
      </ChakraScrollArea.Viewport>
      {orientation !== "horizontal" && <ChakraScrollArea.Scrollbar orientation="vertical" />}
      {orientation !== "vertical" && <ChakraScrollArea.Scrollbar orientation="horizontal" />}
      {orientation === "both" && <ChakraScrollArea.Corner />}
    </ChakraScrollArea.Root>
  );
}
