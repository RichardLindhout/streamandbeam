import { Toaster as Sonner, type ToasterProps } from "sonner";

function Toaster(props: ToasterProps) {
  return (
    <Sonner
      theme="light"
      position="top-center"
      toastOptions={{
        classNames: {
          toast: "!bg-popover !border-border !text-foreground !rounded-xl !shadow-2xl !text-[22px] !p-5 !border-2",
          actionButton: "!bg-walnut !text-white !font-semibold !rounded-lg !px-4 !h-12 !text-[20px]",
        },
      }}
      {...props}
    />
  );
}

export { Toaster };
