import * as React from "react";
import { Popover as P } from "radix-ui";
import { cn } from "@/lib/utils";

const Popover = P.Root;
const PopoverTrigger = P.Trigger;
const PopoverAnchor = P.Anchor;

function PopoverContent({ className, align = "center", sideOffset = 6, collisionPadding = 8, ...props }: React.ComponentProps<typeof P.Content>) {
  return (
    <P.Portal>
      <P.Content
        align={align}
        sideOffset={sideOffset}
        collisionPadding={collisionPadding}
        className={cn("z-50 w-80 rounded-2xl border-2 bg-popover p-4 text-popover-foreground shadow-2xl outline-none max-md:max-w-[calc(100vw-1rem)]", className)}
        {...props}
      />
    </P.Portal>
  );
}

export { Popover, PopoverTrigger, PopoverAnchor, PopoverContent };
