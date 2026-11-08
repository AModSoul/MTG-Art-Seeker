import { Button, Text } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, screen, userEvent, waitFor } from "storybook/test";

import {
  DrawerBody,
  DrawerCloseTrigger,
  DrawerContent,
  DrawerFooter,
  DrawerHeader,
  DrawerRoot,
  DrawerTitle,
  DrawerTrigger,
} from "./drawer";

const meta = {
  component: DrawerRoot,
  // `render` supplies the children; `null` satisfies the type.
  args: { size: "md", onOpenChange: fn(), children: null },
  render: (args) => (
    <DrawerRoot {...args}>
      <DrawerTrigger asChild>
        <Button variant="outline">Open settings</Button>
      </DrawerTrigger>
      <DrawerContent>
        <DrawerHeader>
          <DrawerTitle>Settings</DrawerTitle>
        </DrawerHeader>
        <DrawerBody>
          <Text>Changes are saved straight away.</Text>
        </DrawerBody>
        <DrawerFooter>
          <Text fontSize="sm" color="fg.muted">
            Version 0.1.0
          </Text>
        </DrawerFooter>
        <DrawerCloseTrigger />
      </DrawerContent>
    </DrawerRoot>
  ),
} satisfies Meta<typeof DrawerRoot>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Opens from its trigger and closes from the X; the content renders in a portal. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Open settings" }));
    const drawer = await screen.findByRole("dialog", { name: "Settings" });
    await waitFor(() => expect(drawer).toBeVisible());
    await expect(args.onOpenChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ open: true }),
    );

    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() =>
      expect(args.onOpenChange).toHaveBeenLastCalledWith(expect.objectContaining({ open: false })),
    );
  },
};

export const FromTheStart: Story = { args: { placement: "start" } };

/** A body taller than the drawer scrolls in a scroll area, between the header and footer. */
export const LongBody: Story = {
  render: (args) => (
    <DrawerRoot {...args}>
      <DrawerTrigger asChild>
        <Button variant="outline">Open settings</Button>
      </DrawerTrigger>
      <DrawerContent>
        <DrawerHeader>
          <DrawerTitle>Settings</DrawerTitle>
        </DrawerHeader>
        <DrawerBody>
          {Array.from({ length: 60 }, (_, index) => (
            <Text key={index} py="1">
              Setting {index + 1}
            </Text>
          ))}
        </DrawerBody>
        <DrawerFooter>
          <Text fontSize="sm" color="fg.muted">
            Version 0.1.0
          </Text>
        </DrawerFooter>
        <DrawerCloseTrigger />
      </DrawerContent>
    </DrawerRoot>
  ),
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Open settings" }));
    const drawer = await screen.findByRole("dialog", { name: "Settings" });
    await waitFor(() => expect(drawer).toBeVisible());
    const viewport = drawer.querySelector<HTMLElement>(
      "[data-scope=scroll-area][data-part=viewport]",
    );
    await expect(viewport).not.toBeNull();
    if (!viewport) return;
    await expect(viewport.scrollHeight).toBeGreaterThan(viewport.clientHeight);
    // The footer stays in view below the scrolling body.
    await expect(screen.getByText("Version 0.1.0")).toBeVisible();
  },
};
