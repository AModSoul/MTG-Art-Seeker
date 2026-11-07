import { Stack } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent } from "storybook/test";

import { Radio, RadioGroup } from "./radio";

const meta = {
  component: RadioGroup,
  args: { defaultValue: "edhrec-top-year", onValueChange: fn() },
  render: (args) => (
    <RadioGroup {...args}>
      <Stack gap="3">
        <Radio value="edhrec-top-year">Top 100, past 2 years</Radio>
        <Radio value="edhrec-top-month">Top 100, past month</Radio>
        <Radio value="edhrec-salt">Saltiest 100</Radio>
      </Stack>
    </RadioGroup>
  ),
} satisfies Meta<typeof RadioGroup>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvas, args }) => {
    await expect(canvas.getByRole("radio", { name: "Top 100, past 2 years" })).toBeChecked();
    await userEvent.click(canvas.getByRole("radio", { name: "Saltiest 100" }));
    await expect(args.onValueChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ value: "edhrec-salt" }),
    );
  },
};

export const Disabled: Story = { args: { disabled: true } };
