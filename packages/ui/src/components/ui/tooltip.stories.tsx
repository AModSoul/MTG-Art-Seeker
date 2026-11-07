import { IconButton } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { LuSettings } from "react-icons/lu";
import { expect, screen, userEvent, waitFor } from "storybook/test";

import { Tooltip } from "./tooltip";

const meta = {
  component: Tooltip,
  args: {
    content: "Settings",
    openDelay: 0,
    children: (
      <IconButton variant="ghost" size="sm" aria-label="Settings">
        <LuSettings />
      </IconButton>
    ),
  },
} satisfies Meta<typeof Tooltip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Shows on hover, in a portal. It doesn't name its trigger: give icon buttons an `aria-label`. */
export const Default: Story = {
  play: async ({ canvas }) => {
    await userEvent.hover(canvas.getByRole("button", { name: "Settings" }));
    await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent("Settings"));
  },
};

export const WithArrow: Story = { args: { showArrow: true } };

/** `disabled` turns the tooltip off and leaves the trigger as it is. */
export const Disabled: Story = { args: { disabled: true } };
