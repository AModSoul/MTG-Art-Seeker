import { RadioGroup as ChakraRadioGroup } from "@chakra-ui/react";

export interface RadioProps extends ChakraRadioGroup.ItemProps {
  rootRef?: React.Ref<HTMLDivElement>;
  inputProps?: React.InputHTMLAttributes<HTMLInputElement>;
  ref?: React.Ref<HTMLInputElement>;
}

export function Radio({ ref, children, inputProps, rootRef, ...rest }: RadioProps) {
  return (
    <ChakraRadioGroup.Item ref={rootRef} {...rest}>
      <ChakraRadioGroup.ItemHiddenInput ref={ref} {...inputProps} />
      <ChakraRadioGroup.ItemIndicator />
      {children && <ChakraRadioGroup.ItemText>{children}</ChakraRadioGroup.ItemText>}
    </ChakraRadioGroup.Item>
  );
}

export const RadioGroup = ChakraRadioGroup.Root;
