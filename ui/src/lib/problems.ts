import type { StreamStatus, View, YouTubeCheck } from "./api";

/** Something the volunteer must act on, in plain words, with what to do about it. */
export type Problem = {
  id: string;
  severity: "rood" | "oranje";
  title: string;
  text: string;
  steps: string[];
  /** The technical detail, only shown inside "Wat moet ik doen?". */
  detail?: string;
};

export function problems(state: View | null, status: StreamStatus, yt: YouTubeCheck | null, connected: boolean): Problem[] {
  const list: Problem[] = [];
  const running = status.mode !== "stop";
  if (!connected) {
    list.push({
      id: "verbinding",
      severity: "rood",
      title: "Geen verbinding",
      text: "De bediening bereikt de kerk-PC niet.",
      steps: [
        "Staat de kerk-PC aan?",
        "Staat het programma Stream & Beam open op de kerk-PC?",
        "Werkt het netwerk (wifi of netwerkkabel)?",
        "Herlaad daarna deze pagina (vernieuwen).",
      ],
    });
    return list;
  }
  if (running && status.audioWarning) {
    list.push({
      id: "geluid",
      severity: "rood",
      title: "Geen geluid",
      text: "Het geluid van de kerk komt niet binnen.",
      steps: [
        "Staat de mengtafel aan, en staat de schuif voor de uitzending open?",
        "Zit de USB-kabel van de geluidskaart goed in de PC?",
        "Zet het geluid op de mengtafel even uit en weer aan.",
        "Helpt het niet? Bel de technische contactpersoon.",
      ],
      detail: status.audioWarning,
    });
  }
  if (status.mode === "live" && yt?.at && yt.live && !yt.audio) {
    list.push({
      id: "youtube-geluid",
      severity: "rood",
      title: "Geen geluid op YouTube",
      text: "Kijkers thuis horen niets.",
      steps: [
        "Kijk naar de geluidsmeter onder het beeld: beweegt die als er gezongen of gesproken wordt?",
        "Beweegt de meter niet: controleer de mengtafel en de USB-kabel van de geluidskaart.",
        "Beweegt de meter wel: meld het aan de technische contactpersoon.",
        "Na ongeveer een minuut wordt opnieuw gecontroleerd.",
      ],
      detail: yt.problem,
    });
  }
  if (running && status.error) {
    list.push({
      id: "uitzending",
      severity: "rood",
      title: "De uitzending hapert",
      text: "Er ging iets mis met het beeld of de verbinding met YouTube.",
      steps: [
        "Wacht tien tellen: het programma probeert het vaak zelf opnieuw.",
        "Werkt het internet in de kerk?",
        "Blijft het haperen? Bel de technische contactpersoon.",
      ],
      detail: status.error,
    });
  }
  if (running && status.boards === 0 && state) {
    list.push({
      id: "borden",
      severity: "oranje",
      title: "Borden niet verbonden",
      text: "De tv's in de kerk krijgen de liturgie niet.",
      steps: [
        "Staan de tv's aan?",
        "Staat op de tv de pagina van het bord open?",
        "Zitten de tv's op het wifi-netwerk van de kerk?",
      ],
    });
  }
  return list;
}
