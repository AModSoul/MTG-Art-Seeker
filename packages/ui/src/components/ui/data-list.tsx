import { DataList as ChakraDataList } from "@chakra-ui/react";

export const DataListRoot = ChakraDataList.Root;

export interface DataListItemProps extends ChakraDataList.ItemProps {
  label: React.ReactNode;
  value: React.ReactNode;
  grow?: boolean;
  ref?: React.Ref<HTMLDivElement>;
}

export function DataListItem({ ref, label, value, children, grow, ...rest }: DataListItemProps) {
  return (
    <ChakraDataList.Item ref={ref} {...rest}>
      <ChakraDataList.ItemLabel flex={grow ? "1" : undefined}>{label}</ChakraDataList.ItemLabel>
      <ChakraDataList.ItemValue flex={grow ? "1" : undefined}>{value}</ChakraDataList.ItemValue>
      {children}
    </ChakraDataList.Item>
  );
}
