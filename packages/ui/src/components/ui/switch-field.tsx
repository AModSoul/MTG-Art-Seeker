import { Field } from "@chakra-ui/react";

import { Switch, type SwitchProps } from "./switch";

type SwitchSize = "xs" | "sm" | "md" | "lg";

/** How far a switch's label is from its start, per size: the track's width and the root's gap,
 * as Chakra's switch recipe sets them. */
const LABEL_OFFSET: Record<SwitchSize, string> = {
  xs: "calc({sizes.6} + {spacing.2.5})",
  sm: "calc({sizes.8} + {spacing.2.5})",
  md: "calc({sizes.10} + {spacing.2.5})",
  lg: "calc({sizes.12} + {spacing.2.5})",
};

export interface SwitchFieldProps extends Omit<SwitchProps, "children" | "size" | "label"> {
  label: React.ReactNode;
  /** A line under the label, lined up with it, that describes the switch. */
  helperText?: React.ReactNode;
  size?: SwitchSize;
}

/** A switch with help text under its label. */
export function SwitchField({ label, helperText, size = "md", ...rest }: SwitchFieldProps) {
  return (
    <Field.Root>
      <Switch size={size} {...rest}>
        {label}
      </Switch>
      {helperText && <Field.HelperText ps={LABEL_OFFSET[size]}>{helperText}</Field.HelperText>}
    </Field.Root>
  );
}
