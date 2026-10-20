import * as React from "react";
import { Tabs as P } from "radix-ui";
import { cn } from "@/lib/utils";

const Tabs = P.Root;

function TabsList({ className, ...props }: React.ComponentProps<typeof P.List>) {
  return <P.List className={cn("flex gap-1 overflow-x-auto border-b px-6", className)} {...props} />;
}

function TabsTrigger({ className, ...props }: React.ComponentProps<typeof P.Trigger>) {
  return (
    <P.Trigger
      className={cn(
        "-mb-px border-b-2 border-transparent px-3 py-3 text-sm font-semibold whitespace-nowrap text-muted-foreground transition hover:text-foreground data-[state=active]:border-primary data-[state=active]:text-foreground",
        className,
      )}
      {...props}
    />
  );
}

const TabsContent = P.Content;

export { Tabs, TabsList, TabsTrigger, TabsContent };
