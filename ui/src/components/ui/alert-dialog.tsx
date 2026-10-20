import * as React from "react";
import { AlertDialog as P } from "radix-ui";
import { cn } from "@/lib/utils";
import { buttonVariants } from "./button";

const AlertDialog = P.Root;
const AlertDialogTrigger = P.Trigger;

function AlertDialogContent({ className, ...props }: React.ComponentProps<typeof P.Content>) {
  return (
    <P.Portal>
      <P.Overlay className="fixed inset-0 z-50 bg-[#2b1d12]/55" />
      <P.Content
        className={cn("fixed top-1/2 left-1/2 z-50 grid w-[calc(100%-2rem)] max-w-xl -translate-x-1/2 -translate-y-1/2 gap-7 rounded-2xl border-2 bg-popover p-8 shadow-2xl", className)}
        {...props}
      />
    </P.Portal>
  );
}

function AlertDialogHeader({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("grid gap-2", className)} {...props} />;
}
function AlertDialogFooter({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("grid grid-cols-2 gap-4", className)} {...props} />;
}
function AlertDialogTitle({ className, ...props }: React.ComponentProps<typeof P.Title>) {
  return <P.Title className={cn("font-serif text-[32px] leading-tight font-semibold tracking-tight", className)} {...props} />;
}
function AlertDialogDescription({ className, ...props }: React.ComponentProps<typeof P.Description>) {
  return <P.Description className={cn("text-[22px] leading-snug text-foreground", className)} {...props} />;
}
function AlertDialogAction({ className, ...props }: React.ComponentProps<typeof P.Action>) {
  return <P.Action className={cn(buttonVariants({ size: "lg" }), className)} {...props} />;
}
function AlertDialogCancel({ className, ...props }: React.ComponentProps<typeof P.Cancel>) {
  return <P.Cancel className={cn(buttonVariants({ variant: "outline", size: "lg" }), className)} {...props} />;
}

export { AlertDialog, AlertDialogTrigger, AlertDialogContent, AlertDialogHeader, AlertDialogFooter, AlertDialogTitle, AlertDialogDescription, AlertDialogAction, AlertDialogCancel };
