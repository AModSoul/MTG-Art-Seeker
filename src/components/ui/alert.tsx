import { Alert as ChakraAlert } from "@chakra-ui/react";

export interface AlertProps extends Omit<ChakraAlert.RootProps, "title"> {
  startElement?: React.ReactNode;
  endElement?: React.ReactNode;
  title?: React.ReactNode;
  icon?: React.ReactElement;
  ref?: React.Ref<HTMLDivElement>;
}

export function Alert({
  ref,
  title,
  children,
  icon,
  startElement,
  endElement,
  ...rest
}: AlertProps) {
  return (
    <ChakraAlert.Root ref={ref} {...rest}>
      {startElement || <ChakraAlert.Indicator>{icon}</ChakraAlert.Indicator>}
      {children ? (
        <ChakraAlert.Content>
          <ChakraAlert.Title>{title}</ChakraAlert.Title>
          <ChakraAlert.Description>{children}</ChakraAlert.Description>
        </ChakraAlert.Content>
      ) : (
        <ChakraAlert.Title flex="1">{title}</ChakraAlert.Title>
      )}
      {endElement}
    </ChakraAlert.Root>
  );
}
