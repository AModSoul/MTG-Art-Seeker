import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { SwitchField } from "./switch-field";

/** The help text's padding puts its first letter under the label's. */
async function expectHelpUnderLabel(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  const label = canvas.getByText("Replace files with the same name");
  const help = canvas.getByText("Otherwise a new file is numbered: Name (1).jpg");
  const helpStart =
    help.getBoundingClientRect().left + Number.parseFloat(getComputedStyle(help).paddingLeft);
  await expect(helpStart).toBeCloseTo(label.getBoundingClientRect().left, 0);
}

const meta = {
  component: SwitchField,
  args: {
    label: "Replace files with the same name",
    helperText: "Otherwise a new file is numbered: Name (1).jpg",
    onCheckedChange: fn(),
  },
} satisfies Meta<typeof SwitchField>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Named by its label and described by its help text, which starts where the label does. */
export const Default: Story = {
  play: async ({ canvas, canvasElement, args }) => {
    const toggle = canvas.getByRole("checkbox", { name: "Replace files with the same name" });
    await expect(toggle).toHaveAccessibleDescription(
      "Otherwise a new file is numbered: Name (1).jpg",
    );
    await expectHelpUnderLabel(canvasElement);

    await userEvent.click(toggle);
    await expect(args.onCheckedChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ checked: true }),
    );
  },
};

export const WithoutHelpText: Story = { args: { helperText: undefined } };

/** Each size lines its help text up with its own label. */
export const Small: Story = {
  args: { size: "sm" },
  play: async ({ canvasElement }) => expectHelpUnderLabel(canvasElement),
};
