import { useSyncExternalStore } from "react";

/** The same width as Tailwind's `max-md:` variant: below 48rem the console is laid out for a phone. */
const QUERY = "(max-width: 47.99rem)";

function subscribe(cb: () => void) {
  const m = matchMedia(QUERY);
  m.addEventListener("change", cb);
  return () => m.removeEventListener("change", cb);
}

/** True on a phone-sized screen, for the few layouts CSS alone cannot switch. */
export function usePhone() {
  return useSyncExternalStore(subscribe, () => matchMedia(QUERY).matches);
}
