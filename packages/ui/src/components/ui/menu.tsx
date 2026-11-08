import { AbsoluteCenter, Menu as ChakraMenu, Portal } from "@chakra-ui/react";
import { LuCheck, LuChevronRight } from "react-icons/lu";

interface MenuContentProps extends ChakraMenu.ContentProps {
  portalled?: boolean;
  portalRef?: React.RefObject<HTMLElement | null>;
  ref?: React.Ref<HTMLDivElement>;
}

export function MenuContent({ ref, portalled = true, portalRef, ...rest }: MenuContentProps) {
  return (
    <Portal disabled={!portalled} container={portalRef}>
      <ChakraMenu.Positioner>
        <ChakraMenu.Content ref={ref} {...rest} />
      </ChakraMenu.Positioner>
    </Portal>
  );
}

export function MenuArrow({
  ref,
  ...props
}: ChakraMenu.ArrowProps & { ref?: React.Ref<HTMLDivElement> }) {
  return (
    <ChakraMenu.Arrow ref={ref} {...props}>
      <ChakraMenu.ArrowTip />
    </ChakraMenu.Arrow>
  );
}

export function MenuCheckboxItem({
  ref,
  ...props
}: ChakraMenu.CheckboxItemProps & { ref?: React.Ref<HTMLDivElement> }) {
  return (
    <ChakraMenu.CheckboxItem ps="8" ref={ref} {...props}>
      <AbsoluteCenter axis="horizontal" insetStart="4" asChild>
        <ChakraMenu.ItemIndicator>
          <LuCheck />
        </ChakraMenu.ItemIndicator>
      </AbsoluteCenter>
      {props.children}
    </ChakraMenu.CheckboxItem>
  );
}

export function MenuRadioItem({
  ref,
  children,
  ...rest
}: ChakraMenu.RadioItemProps & { ref?: React.Ref<HTMLDivElement> }) {
  return (
    <ChakraMenu.RadioItem ps="8" ref={ref} {...rest}>
      <AbsoluteCenter axis="horizontal" insetStart="4" asChild>
        <ChakraMenu.ItemIndicator>
          <LuCheck />
        </ChakraMenu.ItemIndicator>
      </AbsoluteCenter>
      <ChakraMenu.ItemText>{children}</ChakraMenu.ItemText>
    </ChakraMenu.RadioItem>
  );
}

export function MenuItemGroup({
  ref,
  title,
  children,
  ...rest
}: ChakraMenu.ItemGroupProps & { ref?: React.Ref<HTMLDivElement> }) {
  return (
    <ChakraMenu.ItemGroup ref={ref} {...rest}>
      {title && <ChakraMenu.ItemGroupLabel userSelect="none">{title}</ChakraMenu.ItemGroupLabel>}
      {children}
    </ChakraMenu.ItemGroup>
  );
}

export interface MenuTriggerItemProps extends ChakraMenu.ItemProps {
  startIcon?: React.ReactNode;
  ref?: React.Ref<HTMLDivElement>;
}

export function MenuTriggerItem({ ref, startIcon, children, ...rest }: MenuTriggerItemProps) {
  return (
    <ChakraMenu.TriggerItem ref={ref} {...rest}>
      {startIcon}
      {children}
      <LuChevronRight />
    </ChakraMenu.TriggerItem>
  );
}

export const MenuRadioItemGroup = ChakraMenu.RadioItemGroup;
export const MenuContextTrigger = ChakraMenu.ContextTrigger;
export const MenuRoot = ChakraMenu.Root;
export const MenuSeparator = ChakraMenu.Separator;

export const MenuItem = ChakraMenu.Item;
export const MenuItemText = ChakraMenu.ItemText;
export const MenuItemCommand = ChakraMenu.ItemCommand;
export const MenuTrigger = ChakraMenu.Trigger;
