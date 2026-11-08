import { SimpleGrid } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { LuPackage } from "react-icons/lu";
import { expect, fn, userEvent } from "storybook/test";

import { RadioCardItem, RadioCardLabel, RadioCardRoot } from "./radio-card";

const DECKS = [
  { value: "abzan", name: "Abzan Armor", set: "Tarkir: Dragonstorm Commander" },
  { value: "world", name: "World Shaper", set: "Edge of Eternities Commander" },
  { value: "minions", name: "Arzakon's Minions", set: "Secret Lair Drop" },
];

const meta = {
  component: RadioCardRoot,
  args: { defaultValue: "abzan", onValueChange: fn() },
  render: (args) => (
    <RadioCardRoot {...args}>
      <RadioCardLabel>Precon decks</RadioCardLabel>
      <SimpleGrid columns={3} gap="2">
        {DECKS.map((deck) => (
          <RadioCardItem
            key={deck.value}
            value={deck.value}
            label={deck.name}
            description={deck.set}
          />
        ))}
      </SimpleGrid>
    </RadioCardRoot>
  ),
} satisfies Meta<typeof RadioCardRoot>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A radio group whose options are cards: one is picked at a time. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    await expect(canvas.getByRole("radiogroup", { name: "Precon decks" })).toBeVisible();
    await expect(canvas.getByRole("radio", { name: /Abzan Armor/ })).toBeChecked();
    await userEvent.click(canvas.getByRole("radio", { name: /World Shaper/ }));
    await expect(args.onValueChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ value: "world" }),
    );
  },
};

/** Stacked, with a picture and no indicator, as the desktop app's decks are. */
export const Vertical: Story = {
  args: { orientation: "vertical", size: "sm" },
  render: (args) => (
    <RadioCardRoot {...args}>
      <RadioCardLabel srOnly>Precon decks</RadioCardLabel>
      <SimpleGrid columns={3} gap="2">
        {DECKS.map((deck) => (
          <RadioCardItem
            key={deck.value}
            value={deck.value}
            indicator={null}
            icon={<LuPackage />}
            label={deck.name}
            description={deck.set}
          />
        ))}
      </SimpleGrid>
    </RadioCardRoot>
  ),
};

export const Disabled: Story = { args: { disabled: true } };
