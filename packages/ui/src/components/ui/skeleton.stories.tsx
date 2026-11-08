import { HStack, Stack, Text } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";

import { Skeleton, SkeletonCircle, SkeletonText } from "./skeleton";

const meta = {
  component: Skeleton,
  args: { w: "16", h: "12", rounded: "sm" },
} satisfies Meta<typeof Skeleton>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A placeholder the size of what's coming, here a thumbnail. */
export const Default: Story = {};

/** With children, it takes their size and fades them in once `loading` is false. */
export const WithChildren: Story = {
  args: { loading: false, w: "auto", h: "auto" },
  render: (args) => (
    <Skeleton {...args}>
      <Text>Karn, Scion of Urza</Text>
    </Skeleton>
  ),
  play: async ({ canvas }) => {
    await waitFor(() => expect(canvas.getByText("Karn, Scion of Urza")).toBeVisible());
  },
};

/** Lines of text, the last one shorter. */
export const Lines: Story = {
  render: () => <SkeletonText noOfLines={3} maxW="sm" />,
  play: async ({ canvasElement }) => {
    await expect(canvasElement.querySelectorAll(".chakra-skeleton")).toHaveLength(3);
  },
};

export const Circle: Story = {
  render: () => (
    <HStack gap="4" maxW="sm">
      <SkeletonCircle size="10" />
      <Stack flex="1">
        <SkeletonText noOfLines={2} />
      </Stack>
    </HStack>
  ),
};
