import { Badge, Box, Button, HStack, Image, SimpleGrid, Stack, Text } from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useState } from "react";

import type { Doubt } from "@/bindings/Doubt";
import type { PageChoice } from "@/bindings/PageChoice";
import {
  DialogBody,
  DialogCloseTrigger,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogRoot,
  DialogTitle,
} from "@mtg-art/ui/dialog";
import { choosePageImage, useRun } from "@/store";

/** What the check said of each image, as a badge. */
function verdictLook(doubt: Doubt | null): { palette: string; label: string } {
  switch (doubt) {
    case null:
      return { palette: "green", label: "The art" };
    case "smaller":
      return { palette: "orange", label: "Not larger" };
    case "cardScan":
      return { palette: "orange", label: "Card scan" };
    case "different":
      return { palette: "gray", label: "Other image" };
  }
}

/** The images of the page open in the finder window, when the user asks to
 * find the art on it: each with its size and what the check said, for them to
 * choose the one to save. */
export function PageImagesDialog() {
  const pageImages = useRun((state) => state.pageImages);
  // The last images stay while the dialog animates out, so it keeps its title.
  const [shown, setShown] = useState(pageImages);
  if (pageImages !== null && pageImages !== shown) setShown(pageImages);

  return (
    <DialogRoot
      size="xl"
      scrollBehavior="inside"
      open={pageImages !== null}
      onOpenChange={(event) => {
        // Escape, the close button or a click outside: save none.
        if (!event.open) void choosePageImage(null);
      }}
    >
      <DialogContent>
        {shown && (
          <>
            <DialogHeader>
              <DialogTitle>Choose {shown.face}'s art</DialogTitle>
            </DialogHeader>
            <DialogBody>
              <Text color="fg.muted" mb="4">
                The images on the page, largest first, with the ones that look like a larger copy of
                Scryfall's art at the top.
              </Text>
              <SimpleGrid minChildWidth="56" gap="4">
                {shown.images.map((image, index) => (
                  <PageImageCard
                    key={image.path}
                    image={image}
                    onSave={() => void choosePageImage(index)}
                  />
                ))}
              </SimpleGrid>
            </DialogBody>
            <DialogFooter>
              <Button variant="outline" onClick={() => void choosePageImage(null)}>
                Cancel
              </Button>
            </DialogFooter>
            <DialogCloseTrigger />
          </>
        )}
      </DialogContent>
    </DialogRoot>
  );
}

function PageImageCard({ image, onSave }: { image: PageChoice; onSave: () => void }) {
  const { palette, label } = verdictLook(image.doubt);
  const size = `${image.size.width} × ${image.size.height}`;
  return (
    <Stack gap="2" borderWidth="1px" rounded="md" p="2">
      <Box bg="bg.subtle" rounded="sm">
        <Image
          src={convertFileSrc(image.path)}
          alt={`${size} image from ${image.host}`}
          w="full"
          h="40"
          objectFit="contain"
        />
      </Box>
      <HStack justify="space-between">
        <Text fontWeight="medium" fontVariantNumeric="tabular-nums">
          {size}
        </Text>
        <Badge colorPalette={palette} variant="subtle">
          {label}
        </Badge>
      </HStack>
      <Text fontSize="xs" color="fg.muted" truncate title={image.host}>
        {image.host}
      </Text>
      <Button
        size="sm"
        variant={image.doubt === null ? "solid" : "outline"}
        aria-label={`Save this one: ${size}, from ${image.host}`}
        onClick={onSave}
      >
        Save this one
      </Button>
    </Stack>
  );
}
