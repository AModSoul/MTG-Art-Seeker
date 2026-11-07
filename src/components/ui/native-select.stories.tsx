import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent } from "storybook/test";

import { Field } from "./field";
import { NativeSelectField, NativeSelectRoot } from "./native-select";

const ITEMS = [
  { value: "mtgpicsThenScryfall", label: "MTGPics, then Scryfall" },
  { value: "mtgpicsOnly", label: "MTGPics only" },
  { value: "scryfallOnly", label: "Scryfall only" },
];

const meta = {
  component: NativeSelectField,
  args: { items: ITEMS, defaultValue: "mtgpicsThenScryfall", onChange: fn() },
  render: (args) => (
    <Field label="Where art comes from">
      <NativeSelectRoot>
        <NativeSelectField {...args} />
      </NativeSelectRoot>
    </Field>
  ),
} satisfies Meta<typeof NativeSelectField>;

export default meta;
type Story = StoryObj<typeof meta>;

/** `items` takes `{ value, label }` objects or plain strings. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    const select = canvas.getByRole("combobox", { name: "Where art comes from" });
    await userEvent.selectOptions(select, "Scryfall only");
    await expect(select).toHaveValue("scryfallOnly");
    await expect(args.onChange).toHaveBeenCalled();
  },
};

export const StringItems: Story = { args: { items: ["Newest", "Oldest"], defaultValue: "Newest" } };
