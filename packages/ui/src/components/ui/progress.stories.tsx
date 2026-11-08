import { HStack } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

import { ProgressBar, ProgressLabel, ProgressRoot, ProgressValueText } from "./progress";

const meta = {
  component: ProgressRoot,
  args: { value: 4, max: 7, size: "sm" },
  render: (args) => (
    <ProgressRoot {...args} maxW="md">
      <HStack justify="space-between" mb="1">
        <ProgressLabel>Downloading</ProgressLabel>
        <ProgressValueText />
      </HStack>
      <ProgressBar />
    </ProgressRoot>
  ),
} satisfies Meta<typeof ProgressRoot>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "4");
  },
};

/** `value: null` animates without a known end. */
export const Indeterminate: Story = { args: { value: null } };
