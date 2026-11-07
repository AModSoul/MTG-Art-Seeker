import { Button, Text } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, screen, userEvent, waitFor } from "storybook/test";

import {
  DialogActionTrigger,
  DialogBody,
  DialogCloseTrigger,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogRoot,
  DialogTitle,
  DialogTrigger,
} from "./dialog";

const meta = {
  component: DialogRoot,
  // `render` supplies the children; `null` satisfies the type.
  args: { onOpenChange: fn(), children: null },
  render: (args) => (
    <DialogRoot {...args}>
      <DialogTrigger asChild>
        <Button variant="outline">Save the image</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Save it anyway?</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <Text>It doesn't look like this card's art.</Text>
        </DialogBody>
        <DialogFooter>
          <DialogActionTrigger asChild>
            <Button variant="outline">Don't save</Button>
          </DialogActionTrigger>
          <Button>Save anyway</Button>
        </DialogFooter>
        <DialogCloseTrigger />
      </DialogContent>
    </DialogRoot>
  ),
} satisfies Meta<typeof DialogRoot>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Opens from its trigger and closes from an action; the content renders in a portal. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Save the image" }));
    const dialog = await screen.findByRole("dialog", { name: "Save it anyway?" });
    await waitFor(() => expect(dialog).toBeVisible());

    await userEvent.click(screen.getByRole("button", { name: "Don't save" }));
    await waitFor(() =>
      expect(args.onOpenChange).toHaveBeenLastCalledWith(expect.objectContaining({ open: false })),
    );
  },
};

/** A question that needs an answer: an alert dialog, which doesn't close from outside. */
export const Alert: Story = {
  args: { role: "alertdialog", closeOnInteractOutside: false },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Save the image" }));
    await waitFor(() =>
      expect(screen.getByRole("alertdialog", { name: "Save it anyway?" })).toBeVisible(),
    );
  },
};
