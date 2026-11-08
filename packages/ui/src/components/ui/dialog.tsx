import { Dialog as ChakraDialog, Portal } from "@chakra-ui/react";

import { CloseButton } from "./close-button";
import { ScrollArea } from "./scroll-area";

interface DialogContentProps extends ChakraDialog.ContentProps {
  portalled?: boolean;
  portalRef?: React.RefObject<HTMLElement | null>;
  backdrop?: boolean;
  ref?: React.Ref<HTMLDivElement>;
}

export function DialogContent({
  ref,
  children,
  portalled = true,
  portalRef,
  backdrop = true,
  ...rest
}: DialogContentProps) {
  return (
    <Portal disabled={!portalled} container={portalRef}>
      {backdrop && <ChakraDialog.Backdrop />}
      <ChakraDialog.Positioner>
        <ChakraDialog.Content ref={ref} {...rest} asChild={false}>
          {children}
        </ChakraDialog.Content>
      </ChakraDialog.Positioner>
    </Portal>
  );
}

export function DialogCloseTrigger({
  ref,
  ...props
}: ChakraDialog.CloseTriggerProps & { ref?: React.Ref<HTMLButtonElement> }) {
  return (
    <ChakraDialog.CloseTrigger position="absolute" top="2" insetEnd="2" {...props} asChild>
      <CloseButton size="sm" ref={ref}>
        {props.children}
      </CloseButton>
    </ChakraDialog.CloseTrigger>
  );
}

export const DialogRoot = ChakraDialog.Root;
export const DialogFooter = ChakraDialog.Footer;
export const DialogHeader = ChakraDialog.Header;
export const DialogBackdrop = ChakraDialog.Backdrop;
export const DialogTitle = ChakraDialog.Title;
export const DialogDescription = ChakraDialog.Description;
export const DialogTrigger = ChakraDialog.Trigger;
export const DialogActionTrigger = ChakraDialog.ActionTrigger;

/** The dialog's body, which scrolls in a scroll area when it's taller than the dialog. Chakra's
 * padding for it is on the scrolling content, so the scrollbar sits at the dialog's edge. */
export function DialogBody({
  ref,
  children,
  ...rest
}: ChakraDialog.BodyProps & { ref?: React.Ref<HTMLDivElement> }) {
  return (
    <ChakraDialog.Body
      ref={ref}
      display="flex"
      flexDirection="column"
      minH="0"
      p="0"
      overflow="hidden"
      {...rest}
    >
      <ScrollArea flex="1" minH="0" contentProps={{ px: "6", pt: "2", pb: "6" }}>
        {children}
      </ScrollArea>
    </ChakraDialog.Body>
  );
}
