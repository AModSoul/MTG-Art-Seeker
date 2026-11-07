import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent } from "storybook/test";

import { Switch } from "./switch";

const meta = {
  component: Switch,
  args: { children: "Replace files with the same name", onCheckedChange: fn() },
} satisfies Meta<typeof Switch>;

export default meta;
type Story = StoryObj<typeof meta>;

/** It's a checkbox to assistive technology, named by its children. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    const toggle = canvas.getByRole("checkbox", { name: "Replace files with the same name" });
    await userEvent.click(toggle);
    await expect(toggle).toBeChecked();
    await expect(args.onCheckedChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ checked: true }),
    );
  },
};

export const Checked: Story = { args: { defaultChecked: true } };

export const Disabled: Story = { args: { disabled: true } };
