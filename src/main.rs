//! Stream & Beam shows the liturgy of HHG Tholen (psalms, readings, sermon theme) on the
//! screens in church. The TVs open http://<this-pc>:8080/beam and follow the operator,
//! who works from http://<this-pc>:8080.

mod audio;
mod books;
mod catechism;
mod claude;
mod compare;
mod confessions;
mod director;
mod excel;
mod extract;
mod kerktijden;
mod layout;
mod listen;
mod parse;
mod render;
mod roosters;
mod show;
mod songs;
mod stream;
mod texts;
mod web;
mod youtube;
mod ytcheck;

use std::path::PathBuf;

struct Options {
    port: u16,
    data_dir: PathBuf,
    liturgy_url: String,
    /// Port for the read-only watch page (for a Cloudflare tunnel); 0 = off (the default).
    public_port: u16,
    command: Option<String>,
    /// What follows the command, e.g. the workbook for importeer-excel.
    args: Vec<String>,
}

fn parse_args() -> Options {
    let mut opts = Options {
        port: 8080,
        data_dir: "data".into(),
        liturgy_url: extract::LITURGY_URL.into(),
        public_port: 0,
        command: None,
        args: Vec::new(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--poort" | "--port" => opts.port = args.next().and_then(|p| p.parse().ok()).unwrap_or(8080),
            "--data" => opts.data_dir = args.next().unwrap_or_default().into(),
            "--liturgie-url" => opts.liturgy_url = args.next().unwrap_or_default(),
            "--publiek-poort" => opts.public_port = args.next().and_then(|p| p.parse().ok()).unwrap_or(0),
            "-h" | "--help" => {
                println!(
                    "streamandbeam [--poort 8080] [--publiek-poort 8090] [--data data] [--liturgie-url URL] [fetch|scenes|importeer-excel <bestand>]\n\n  \
                     --publiek-poort  alleen-kijken-pagina (bord + uitzending) voor internet, bv. 8090; standaard uit\n  \
                     fetch            laat zien wat het programma van de website leest (en de AI-controle, met sleutel)\n  \
                     scenes           schrijf voorbeelden van de uitzendbeelden naar data/scenes\n  \
                     importeer-excel  lees collecte-, voorzang- en bedieningsrooster uit de Excel van de kerk\n                   \
                     (Streamupdate.xlsm) naar data/roosters.json"
                );
                std::process::exit(0);
            }
            other if opts.command.is_none() => opts.command = Some(other.to_string()),
            other => opts.args.push(other.to_string()),
        }
    }
    opts
}

/// `importeer-excel <bestand>`: the schedules from the church's workbook into
/// <data>/roosters.json, with what was found.
fn import_excel(opts: &Options) -> anyhow::Result<()> {
    let Some(file) = opts.args.first() else { anyhow::bail!("gebruik: streamandbeam importeer-excel <bestand.xlsm>") };
    let bytes = std::fs::read(file).map_err(|e| anyhow::anyhow!("{file}: {e}"))?;
    let name = std::path::Path::new(file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let old = roosters::load(&opts.data_dir);
    let (new, report) = roosters::import(&bytes)?.apply(&old, &name, chrono::Local::now().date_naive());
    let out = opts.data_dir.join("roosters.json");
    texts::write_json(&out, &new)?;
    for line in report.lines() {
        println!("{line}");
    }
    println!("Opgeslagen in {}", out.display());
    Ok(())
}

/// The address of this PC on the church network, for the TVs.
fn lan_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.168.0.1:80").map(|_| s))
        .and_then(|s| s.local_addr())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "localhost".into())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = parse_args();
    if opts.command.as_deref() == Some("fetch") {
        let html = extract::http_get(&opts.liturgy_url).await?;
        let mut ex = extract::read_page(&html, chrono::Local::now().date_naive());
        if claude::has_key() {
            match extract::ask_ai(&html).await {
                Ok(ai) => {
                    let (n, notes) = compare::double_check(&mut ex, &ai);
                    println!("AI-controle: {n} verschil(len) {notes:?}");
                }
                Err(e) => println!("AI-controle overgeslagen: {e:#}"),
            }
        }
        println!("{ex:#?}");
        return Ok(());
    }

    if opts.command.as_deref() == Some("scenes") {
        return render::write_examples(&opts.data_dir.join("scenes"));
    }

    if opts.command.as_deref() == Some("importeer-excel") {
        return import_excel(&opts);
    }

    let show = show::Show::new(opts.data_dir.join("liturgie.json"), opts.liturgy_url.clone()).await;
    let engine = stream::Engine::new()?;
    let director = director::Director::new(engine.clone(), &opts.data_dir);
    show.set_slotzang_text(director.settings().slotzang_tekst).await;
    let listener = listen::Listener::new();
    engine.set_listener(listener.clone());
    if director.settings().autopilot != "uit" {
        listener.start();
    }
    let d = director.clone();
    tokio::spawn(listener.clone().suggest_loop(show.clone(), move || d.settings().autopilot));
    let check = ytcheck::YouTubeCheck::new();
    let (d1, d2) = (director.clone(), director.clone());
    tokio::spawn(check.clone().run(move || d1.settings().youtube_channel, move || d2.engine.status().mode == stream::Mode::Live));
    tokio::spawn(director.clone().run(show.clone()));
    // The liturgy from the website: at the start and every hour, not while live (then a few
    // minutes later). The program reads it itself; with a key the AI double-checks it.
    // A failure (no internet) is only shown quietly.
    let (s, d) = (show.clone(), director.clone());
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        loop {
            let live = || d.engine.status().mode == stream::Mode::Live;
            if live() {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                continue;
            }
            match s.fetch(live).await {
                Ok(()) => s.ai_check(live).await,
                Err(e) => eprintln!("website ophalen: {e:#}"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });
    if !claude::has_key() {
        eprintln!("let op: ANTHROPIC_API_KEY is niet gezet; de AI-controle van de liturgie staat uit (het programma leest de website en het invoerveld zelf)");
    }

    let base = format!("http://{}:{}", lan_ip(), opts.port);
    println!("Bediening:          {base}");
    println!("Schermen (TV's):    {base}/beam");
    if opts.public_port > 0 {
        // Only reachable from this computer; the Cloudflare tunnel brings it to the internet.
        let public = web::public_router(show.clone(), director.clone(), listener.clone(), check.clone(), format!("{base}/beam"), opts.port);
        let tcp = tokio::net::TcpListener::bind(("127.0.0.1", opts.public_port)).await?;
        println!("Meekijken (alleen-kijken): http://localhost:{}", opts.public_port);
        tokio::spawn(async move { axum::serve(tcp, public).await });
    }
    let tcp = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;
    axum::serve(tcp, web::router(show, director, listener, check, format!("{base}/beam"), opts.port)).await?;
    Ok(())
}
