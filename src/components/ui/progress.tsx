import { Progress as ChakraProgress } from "@chakra-ui/react";

export function ProgressBar({
  ref,
  ...props
}: ChakraProgress.TrackProps & { ref?: React.Ref<HTMLDivElement> }) {
  return (
    <ChakraProgress.Track {...props} ref={ref}>
      <ChakraProgress.Range />
    </ChakraProgress.Track>
  );
}

export const ProgressRoot = ChakraProgress.Root;
export const ProgressLabel = ChakraProgress.Label;
export const ProgressValueText = ChakraProgress.ValueText;
