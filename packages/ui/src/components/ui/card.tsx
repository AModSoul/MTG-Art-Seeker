import { Card as ChakraCard } from "@chakra-ui/react";

export interface CardProps extends Omit<ChakraCard.RootProps, "title"> {
  /** An image or other media, flush with the card's edges above its body. */
  media?: React.ReactNode;
  /** An icon above the title. */
  icon?: React.ReactNode;
  title?: React.ReactNode;
  description?: React.ReactNode;
  /** Actions, in the card's footer. */
  footer?: React.ReactNode;
  /** The title's props: `as` sets its heading level (`h3` by default). */
  titleProps?: ChakraCard.TitleProps;
  bodyProps?: ChakraCard.BodyProps;
  ref?: React.Ref<HTMLDivElement>;
}

export function Card({
  ref,
  media,
  icon,
  title,
  description,
  footer,
  titleProps,
  bodyProps,
  children,
  ...rest
}: CardProps) {
  return (
    <ChakraCard.Root ref={ref} overflow={media ? "hidden" : undefined} {...rest}>
      {media}
      <ChakraCard.Body gap="2" {...bodyProps}>
        {icon}
        {title && <ChakraCard.Title {...titleProps}>{title}</ChakraCard.Title>}
        {description && <ChakraCard.Description>{description}</ChakraCard.Description>}
        {children}
      </ChakraCard.Body>
      {footer && <ChakraCard.Footer>{footer}</ChakraCard.Footer>}
    </ChakraCard.Root>
  );
}
