import { EmptyState as ChakraEmptyState, VStack } from "@chakra-ui/react";

export interface EmptyStateProps extends ChakraEmptyState.RootProps {
  title: string;
  description?: string;
  icon?: React.ReactNode;
  ref?: React.Ref<HTMLDivElement>;
}

export function EmptyState({ ref, title, description, icon, children, ...rest }: EmptyStateProps) {
  return (
    <ChakraEmptyState.Root ref={ref} {...rest}>
      <ChakraEmptyState.Content>
        {icon && <ChakraEmptyState.Indicator>{icon}</ChakraEmptyState.Indicator>}
        {description ? (
          <VStack textAlign="center">
            <ChakraEmptyState.Title>{title}</ChakraEmptyState.Title>
            <ChakraEmptyState.Description>{description}</ChakraEmptyState.Description>
          </VStack>
        ) : (
          <ChakraEmptyState.Title>{title}</ChakraEmptyState.Title>
        )}
        {children}
      </ChakraEmptyState.Content>
    </ChakraEmptyState.Root>
  );
}
