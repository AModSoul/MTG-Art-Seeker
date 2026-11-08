import type { IconButtonProps, SpanProps } from "@chakra-ui/react";
import { ClientOnly, IconButton, Skeleton, Span } from "@chakra-ui/react";
import { ThemeProvider, useTheme } from "next-themes";
import type { ThemeProviderProps } from "next-themes";
import { LuMoon, LuSun } from "react-icons/lu";

export type ColorModeProviderProps = ThemeProviderProps;

export function ColorModeProvider(props: ColorModeProviderProps) {
  return <ThemeProvider attribute="class" disableTransitionOnChange {...props} />;
}

export type ColorMode = "light" | "dark";

export interface UseColorModeReturn {
  colorMode: ColorMode;
  setColorMode: (colorMode: ColorMode) => void;
  toggleColorMode: () => void;
}

export function useColorMode(): UseColorModeReturn {
  const { resolvedTheme, setTheme, forcedTheme } = useTheme();
  const theme = forcedTheme || resolvedTheme;
  return {
    colorMode: theme === "dark" ? "dark" : "light",
    setColorMode: setTheme,
    toggleColorMode: () => setTheme(resolvedTheme === "dark" ? "light" : "dark"),
  };
}

export function useColorModeValue<T>(light: T, dark: T) {
  const { colorMode } = useColorMode();
  return colorMode === "dark" ? dark : light;
}

export function ColorModeIcon() {
  const { colorMode } = useColorMode();
  return colorMode === "dark" ? <LuMoon /> : <LuSun />;
}

type ColorModeButtonSize = "2xs" | "xs" | "sm";

/** Each size's side, for the square that stands in for the button until it renders. */
const SIDES: Record<ColorModeButtonSize, string> = { "2xs": "6", xs: "8", sm: "9" };

type ColorModeButtonProps = Omit<IconButtonProps, "aria-label" | "size"> & {
  ref?: React.Ref<HTMLButtonElement>;
  /** How large the button is: `sm`, with a 20px icon, unless a bar of small buttons, such as
   * the desktop app's header, asks for `xs` or `2xs`, whose icons are the button's own size
   * (16px and 14px). */
  size?: ColorModeButtonSize;
};

export function ColorModeButton({ ref, size = "sm", ...props }: ColorModeButtonProps) {
  const { toggleColorMode } = useColorMode();
  return (
    <ClientOnly fallback={<Skeleton boxSize={SIDES[size]} />}>
      <IconButton
        onClick={toggleColorMode}
        variant="ghost"
        aria-label="Toggle color mode"
        size={size}
        ref={ref}
        {...props}
        css={size === "sm" ? { _icon: { width: "5", height: "5" } } : undefined}
      >
        <ColorModeIcon />
      </IconButton>
    </ClientOnly>
  );
}

export function LightMode({ ref, ...props }: SpanProps & { ref?: React.Ref<HTMLSpanElement> }) {
  return (
    <Span
      color="fg"
      display="contents"
      className="chakra-theme light"
      colorPalette="gray"
      colorScheme="light"
      ref={ref}
      {...props}
    />
  );
}

export function DarkMode({ ref, ...props }: SpanProps & { ref?: React.Ref<HTMLSpanElement> }) {
  return (
    <Span
      color="fg"
      display="contents"
      className="chakra-theme dark"
      colorPalette="gray"
      colorScheme="dark"
      ref={ref}
      {...props}
    />
  );
}
