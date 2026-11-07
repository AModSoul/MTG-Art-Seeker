import { For, SegmentGroup } from "@chakra-ui/react";
import { useMemo } from "react";

interface Item {
  value: string;
  label: React.ReactNode;
  disabled?: boolean;
}

export interface SegmentedControlProps extends SegmentGroup.RootProps {
  items: Array<string | Item>;
  ref?: React.Ref<HTMLDivElement>;
}

function normalize(items: Array<string | Item>): Item[] {
  return items.map((item) => (typeof item === "string" ? { value: item, label: item } : item));
}

export function SegmentedControl({ ref, items, ...rest }: SegmentedControlProps) {
  const data = useMemo(() => normalize(items), [items]);

  return (
    <SegmentGroup.Root ref={ref} {...rest}>
      <SegmentGroup.Indicator />
      <For each={data}>
        {(item) => (
          <SegmentGroup.Item key={item.value} value={item.value} disabled={item.disabled}>
            <SegmentGroup.ItemText>{item.label}</SegmentGroup.ItemText>
            <SegmentGroup.ItemHiddenInput />
          </SegmentGroup.Item>
        )}
      </For>
    </SegmentGroup.Root>
  );
}
