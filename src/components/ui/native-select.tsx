import { NativeSelect as Select } from "@chakra-ui/react";
import { useMemo } from "react";

interface NativeSelectRootProps extends Select.RootProps {
  icon?: React.ReactNode;
  ref?: React.Ref<HTMLDivElement>;
}

export function NativeSelectRoot({ ref, icon, children, ...rest }: NativeSelectRootProps) {
  return (
    <Select.Root ref={ref} {...rest}>
      {children}
      <Select.Indicator>{icon}</Select.Indicator>
    </Select.Root>
  );
}

interface NativeSelectItem {
  value: string;
  label: string;
  disabled?: boolean;
}

interface NativeSelectFieldProps extends Select.FieldProps {
  items?: Array<string | NativeSelectItem>;
  ref?: React.Ref<HTMLSelectElement>;
}

export function NativeSelectField({
  ref,
  items: itemsProp,
  children,
  ...rest
}: NativeSelectFieldProps) {
  const items = useMemo(
    () =>
      itemsProp?.map((item) => (typeof item === "string" ? { label: item, value: item } : item)),
    [itemsProp],
  );

  return (
    <Select.Field ref={ref} {...rest}>
      {children}
      {items?.map((item) => (
        <option key={item.value} value={item.value} disabled={item.disabled}>
          {item.label}
        </option>
      ))}
    </Select.Field>
  );
}
