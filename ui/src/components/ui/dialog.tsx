import * as React from "react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { XIcon } from "lucide-react";
import { cn } from "@/lib/utils";

const Dialog = DialogPrimitive.Root;
const DialogTrigger = DialogPrimitive.Trigger;
const DialogClose = DialogPrimitive.Close;

function DialogOverlay({ className, ...props }: React.ComponentProps<typeof DialogPrimitive.Overlay>) {
  return <DialogPrimitive.Overlay className={cn("fixed inset-0 z-50 bg-[#2b1d12]/55", className)} {...props} />;
}

function DialogContent({ className, children, ...props }: React.ComponentProps<typeof DialogPrimitive.Content>) {
  return (
    <DialogPrimitive.Portal>
      <DialogOverlay />
      <DialogPrimitive.Content
        className={cn(
          "fixed top-1/2 left-1/2 z-50 flex max-h-[92vh] w-[calc(100%-2rem)] max-w-xl -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-2xl border-2 bg-popover shadow-2xl outline-none",
          // On a phone: the whole screen, the body scrolls and the buttons stay at the bottom (above the on-screen keyboard).
          "max-md:inset-0 max-md:h-[100dvh] max-md:max-h-none max-md:w-full max-md:max-w-none max-md:translate-x-0 max-md:translate-y-0 max-md:rounded-none max-md:border-0 max-md:pt-[env(safe-area-inset-top)]",
          className,
        )}
        {...props}
      >
        {children}
        <DialogPrimitive.Close className="absolute top-3 right-3 grid size-14 place-items-center rounded-xl text-foreground transition hover:bg-accent max-md:top-[calc(0.5rem+env(safe-area-inset-top))] max-md:right-2">
          <XIcon className="size-7" />
          <span className="sr-only">Sluiten</span>
        </DialogPrimitive.Close>
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
}

function DialogHeader({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("flex flex-col gap-1 border-b py-5 pr-20 pl-7 max-md:min-h-[4.5rem] max-md:shrink-0 max-md:justify-center max-md:py-3 max-md:pr-16 max-md:pl-4", className)} {...props} />;
}

function DialogBody({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("scroll-thin grid gap-5 overflow-y-auto px-7 py-6 max-md:grid-cols-[minmax(0,1fr)] max-md:min-h-0 max-md:flex-1 max-md:content-start max-md:px-4 max-md:py-4", className)} {...props} />;
}

function DialogFooter({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("flex flex-wrap justify-end gap-3 border-t bg-background/60 px-7 py-4 max-md:shrink-0 max-md:px-4 max-md:py-3 max-md:pb-[max(0.75rem,env(safe-area-inset-bottom))] max-md:[&>*]:flex-1", className)} {...props} />;
}

function DialogTitle({ className, ...props }: React.ComponentProps<typeof DialogPrimitive.Title>) {
  return <DialogPrimitive.Title className={cn("font-serif text-[30px] leading-tight font-semibold tracking-tight max-md:text-[24px]", className)} {...props} />;
}

function DialogDescription({ className, ...props }: React.ComponentProps<typeof DialogPrimitive.Description>) {
  return <DialogPrimitive.Description className={cn("text-[20px] text-muted-foreground", className)} {...props} />;
}

export { Dialog, DialogTrigger, DialogClose, DialogContent, DialogHeader, DialogBody, DialogFooter, DialogTitle, DialogDescription };
