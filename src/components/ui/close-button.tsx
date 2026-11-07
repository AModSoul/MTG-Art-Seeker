import type { ButtonProps } from "@chakra-ui/react";
import { IconButton as ChakraIconButton } from "@chakra-ui/react";
import { LuX } from "react-icons/lu";

export type CloseButtonProps = ButtonProps & { ref?: React.Ref<HTMLButtonElement> };

export function CloseButton({ ref, ...props }: CloseButtonProps) {
  return (
    <ChakraIconButton variant="ghost" aria-label="Close" ref={ref} {...props}>
      {props.children ?? <LuX />}
    </ChakraIconButton>
  );
}
