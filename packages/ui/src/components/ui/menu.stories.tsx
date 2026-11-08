import { Box, Button } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, screen, userEvent, waitFor } from "storybook/test";

import {
  MenuContent,
  MenuContextTrigger,
  MenuItem,
  MenuItemGroup,
  MenuRoot,
  MenuSeparator,
  MenuTrigger,
} from "./menu";

const meta = {
  component: MenuRoot,
  // `render` supplies the children; `null` satisfies the type.
  args: { onSelect: fn(), children: null },
  render: (args) => (
    <MenuRoot {...args}>
      <MenuTrigger asChild>
        <Button variant="outline">Use art from…</Button>
      </MenuTrigger>
      <MenuContent>
        <MenuItemGroup title="Use art from">
          <MenuItem value="mtgpics">MTGPics</MenuItem>
          <MenuItem value="scryfall" disabled>
            Scryfall (saved now)
          </MenuItem>
        </MenuItemGroup>
        <MenuSeparator />
        <MenuItem value="reveal">Show in folder</MenuItem>
      </MenuContent>
    </MenuRoot>
  ),
} satisfies Meta<typeof MenuRoot>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Opens from its trigger; an item reports its value. The content renders in a portal. */
export const Default: Story = {
  play: async ({ args, canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Use art from…" }));
    const item = await screen.findByRole("menuitem", { name: "MTGPics" });
    await waitFor(() => expect(item).toBeVisible());
    await userEvent.click(item);
    await expect(args.onSelect).toHaveBeenCalledWith(expect.objectContaining({ value: "mtgpics" }));
  },
};

/** Opens where the user right-clicks the area it wraps. */
export const ContextMenu: Story = {
  render: (args) => (
    <MenuRoot {...args}>
      <MenuContextTrigger asChild>
        <Box borderWidth="1px" borderStyle="dashed" rounded="md" p="8" w="xs" textAlign="center">
          Right-click here
        </Box>
      </MenuContextTrigger>
      <MenuContent>
        <MenuItem value="mtgpics">MTGPics</MenuItem>
        <MenuItem value="scryfall">Scryfall</MenuItem>
      </MenuContent>
    </MenuRoot>
  ),
  play: async ({ canvas }) => {
    await userEvent.pointer({ keys: "[MouseRight]", target: canvas.getByText("Right-click here") });
    await waitFor(() => expect(screen.getByRole("menuitem", { name: "Scryfall" })).toBeVisible());
  },
};
