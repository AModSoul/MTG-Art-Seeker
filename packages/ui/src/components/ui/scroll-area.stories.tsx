import { Box, Flex, Text } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

import { ScrollArea } from "./scroll-area";

const CARDS = Array.from({ length: 30 }, (_, index) => `Card ${index + 1}`);

const meta = {
  component: ScrollArea,
  args: { height: "40", maxW: "sm", borderWidth: "1px", rounded: "md" },
  render: (args) => (
    <ScrollArea {...args}>
      {CARDS.map((card) => (
        <Text key={card} px="3" py="1">
          {card}
        </Text>
      ))}
    </ScrollArea>
  ),
} satisfies Meta<typeof ScrollArea>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A fixed height: the content scrolls inside it. */
export const Default: Story = {
  play: async ({ canvasElement }) => {
    const viewport = canvasElement.querySelector<HTMLElement>("[data-part=viewport]");
    await expect(viewport).not.toBeNull();
    if (!viewport) return;
    await expect(viewport.scrollHeight).toBeGreaterThan(viewport.clientHeight);
  },
};

/** `flex="1" minH="0"` fills what a flex column leaves, as the run table does. */
export const FillsAFlexColumn: Story = {
  args: { height: undefined, flex: "1", minH: "0", maxW: undefined },
  render: (args) => (
    <Flex direction="column" h="64" maxW="sm" gap="2">
      <Text fontWeight="medium">Above the list</Text>
      <ScrollArea {...args}>
        {CARDS.map((card) => (
          <Text key={card} px="3" py="1">
            {card}
          </Text>
        ))}
      </ScrollArea>
    </Flex>
  ),
};

/** Sideways, for a long line of code. */
export const Horizontal: Story = {
  args: { orientation: "horizontal", height: undefined, maxW: "xs" },
  render: (args) => (
    <ScrollArea {...args}>
      <Box as="pre" p="3" fontSize="sm">
        <code>mtg-art --search "set:mh2 t:creature pow&gt;=3" --out ./art</code>
      </Box>
    </ScrollArea>
  ),
};
