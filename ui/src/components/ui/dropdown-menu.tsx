import * as React from "react";
import { DropdownMenu as P } from "radix-ui";
import { cn } from "@/lib/utils";

const DropdownMenu = P.Root;
const DropdownMenuTrigger = P.Trigger;

function DropdownMenuContent({ className, sideOffset = 6, align = "end", ...props }: React.ComponentProps<typeof P.Content>) {
  return (
    <P.Portal>
      <P.Content
        sideOffset={sideOffset}
        align={align}
        className={cn("z-50 min-w-[22rem] overflow-hidden rounded-2xl border-2 bg-popover p-2 text-popover-foreground shadow-2xl", className)}
        {...props}
      />
    </P.Portal>
  );
}

function DropdownMenuItem({ className, ...props }: React.ComponentProps<typeof P.Item>) {
  return (
    <P.Item
      className={cn(
        "relative flex cursor-default items-center gap-3 min-h-[72px] rounded-xl px-5 text-[28px] outline-none select-none focus:bg-accent data-[disabled]:text-[#9a8a76] [&_svg]:size-7 [&_svg]:text-walnut data-[disabled]:[&_svg]:text-[#b8a88f]",
        className,
      )}
      {...props}
    />
  );
}

function DropdownMenuSeparator({ className, ...props }: React.ComponentProps<typeof P.Separator>) {
  return <P.Separator className={cn("my-1.5 h-0.5 bg-border", className)} {...props} />;
}

export { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator };
