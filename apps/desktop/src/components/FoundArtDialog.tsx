import { Button, Text } from "@chakra-ui/react";
import { useState } from "react";

import type { Doubt } from "@/bindings/Doubt";
import {
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogRoot,
  DialogTitle,
} from "@mtg-art/ui/dialog";
import { keepHeader } from "@/layout";
import { answerDoubt, useRun } from "@/store";

/** What the check found, for each reason an image wasn't saved. */
const DOUBT_LOOK: Record<Doubt, { title: string; body: (face: string) => string }> = {
  smaller: {
    title: "This copy isn't any larger",
    body: (face) => `It looks like ${face}'s art, but it's no bigger than Scryfall's.`,
  },
  cardScan: {
    title: "This looks like a whole card",
    body: (face) => `It shows ${face}'s art inside a card's frame, not the art on its own.`,
  },
  different: {
    title: "This doesn't look like the art",
    body: (face) => `It doesn't match Scryfall's art for ${face}.`,
  },
};

/** Asks about an image the user saved in the Lens window that didn't look like
 * a larger copy of the art: save it anyway, or throw it away. One at a time,
 * oldest first. */
export function FoundArtDialog() {
  const doubt = useRun((state) => state.doubts[0]);
  // The last doubt stays while the dialog animates out, so it keeps its title.
  const [shown, setShown] = useState(doubt);
  if (doubt !== undefined && doubt !== shown) setShown(doubt);
  const look = shown && DOUBT_LOOK[shown.doubt];

  return (
    <DialogRoot
      role="alertdialog"
      open={doubt !== undefined}
      closeOnInteractOutside={false}
      persistentElements={keepHeader}
      onOpenChange={(event) => {
        // Escape, or the window closing it: don't save.
        if (!event.open) void answerDoubt(false);
      }}
    >
      <DialogContent>
        {shown && look && (
          <>
            <DialogHeader>
              <DialogTitle>{look.title}</DialogTitle>
            </DialogHeader>
            <DialogBody>
              <Text>{look.body(shown.face)}</Text>
              <Text mt="2" color="fg.muted">
                Save it as the art anyway?
              </Text>
            </DialogBody>
            <DialogFooter>
              <Button variant="outline" onClick={() => void answerDoubt(false)}>
                Don't save
              </Button>
              <Button onClick={() => void answerDoubt(true)}>Save anyway</Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </DialogRoot>
  );
}
