import "../../fonts.css";
import { ChakraProvider, type SystemContext } from "@chakra-ui/react";
import { system as sharedSystem } from "../../theme";
import { ColorModeProvider, type ColorModeProviderProps } from "./color-mode";

type ProviderProps = ColorModeProviderProps & {
  /** The Chakra system: the shared one, unless an app adds a layer of its own to it. */
  system?: SystemContext;
};

export function Provider({ system = sharedSystem, ...props }: ProviderProps) {
  return (
    <ChakraProvider value={system}>
      <ColorModeProvider {...props} />
    </ChakraProvider>
  );
}
