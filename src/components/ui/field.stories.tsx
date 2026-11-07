import { Input } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

import { Field } from "./field";

const meta = {
  component: Field,
  args: {
    label: "File names",
    children: <Input defaultValue="NAME (ARTIST) [SET] {NUMBER}" />,
  },
} satisfies Meta<typeof Field>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The label is tied to its control. */
export const Default: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByLabelText("File names")).toHaveValue("NAME (ARTIST) [SET] {NUMBER}");
  },
};

export const WithHelperText: Story = {
  args: { helperText: "NAME, ARTIST, SET and NUMBER are filled in." },
};

/** `invalid` shows `errorText` instead of the helper text. */
export const Invalid: Story = {
  args: {
    invalid: true,
    helperText: "NAME, ARTIST, SET and NUMBER are filled in.",
    errorText: "A file name can't be empty.",
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("A file name can't be empty.")).toBeVisible();
    await expect(canvas.getByLabelText(/File names/)).toHaveAttribute("aria-invalid", "true");
  },
};

export const Required: Story = { args: { required: true } };

export const Optional: Story = { args: { optionalText: "(optional)" } };
