import { Drawer as ChakraDrawer, Portal } from "@chakra-ui/react";

import { CloseButton } from "./close-button";
import { ScrollArea } from "./scroll-area";

interface DrawerContentProps extends ChakraDrawer.ContentProps {
  portalled?: boolean;
  portalRef?: React.RefObject<HTMLElement | null>;
  offset?: ChakraDrawer.ContentProps["padding"];
  backdrop?: boolean;
  ref?: React.Ref<HTMLDivElement>;
}

export function DrawerContent({
  ref,
  children,
  portalled = true,
  portalRef,
  offset,
  backdrop = true,
  ...rest
}: DrawerContentProps) {
  return (
    <Portal disabled={!portalled} container={portalRef}>
      {backdrop && <ChakraDrawer.Backdrop />}
      <ChakraDrawer.Positioner padding={offset}>
        <ChakraDrawer.Content ref={ref} {...rest} asChild={false}>
          {children}
        </ChakraDrawer.Content>
      </ChakraDrawer.Positioner>
    </Portal>
  );
}

export function DrawerCloseTrigger({
  ref,
  ...props
}: ChakraDrawer.CloseTriggerProps & { ref?: React.Ref<HTMLButtonElement> }) {
  return (
    <ChakraDrawer.CloseTrigger position="absolute" top="2" insetEnd="2" {...props} asChild>
      <CloseButton size="sm" ref={ref} />
    </ChakraDrawer.CloseTrigger>
  );
}

export const DrawerTrigger = ChakraDrawer.Trigger;
export const DrawerRoot = ChakraDrawer.Root;
export const DrawerFooter = ChakraDrawer.Footer;
export const DrawerHeader = ChakraDrawer.Header;
export const DrawerBackdrop = ChakraDrawer.Backdrop;
export const DrawerDescription = ChakraDrawer.Description;
export const DrawerTitle = ChakraDrawer.Title;
export const DrawerActionTrigger = ChakraDrawer.ActionTrigger;

/** The drawer's body, which scrolls in a scroll area when it's taller than the drawer. Chakra's
 * padding for it is on the scrolling content, so the scrollbar sits at the drawer's edge. */
export function DrawerBody({
  ref,
  children,
  ...rest
}: ChakraDrawer.BodyProps & { ref?: React.Ref<HTMLDivElement> }) {
  return (
    <ChakraDrawer.Body
      ref={ref}
      display="flex"
      flexDirection="column"
      minH="0"
      p="0"
      overflow="hidden"
      {...rest}
    >
      <ScrollArea flex="1" minH="0" contentProps={{ px: "6", py: "2" }}>
        {children}
      </ScrollArea>
    </ChakraDrawer.Body>
  );
}
