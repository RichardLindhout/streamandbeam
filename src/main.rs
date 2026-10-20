//! Stream & Beam shows the liturgy of HHG Tholen (psalms, readings, sermon theme) on the
//! screens in church. The TVs open http://<this-pc>:8080/beam and follow the operator,
//! who works from http://<this-pc>:8080.

mod books;
mod catechism;
mod claude;
mod confessions;
mod director;
mod extract;
mod kerktijden;
mod listen;
mod render;
mod show;
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
    /// Port for the read-only watch page (for a Cloudflare tunnel); 0 = off.
    public_port: u16,
    command: Option<String>,
}

fn parse_args() -> Options {
    let mut opts = Options {
        port: 8080,
        data_dir: "data".into(),
        liturgy_url: extract::LITURGY_URL.into(),
        public_port: 8090,
        command: None,
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
                    "streamandbeam [--poort 8080] [--publiek-poort 8090] [--data data] [--liturgie-url URL] [fetch|scenes]\n\n  \
                     --publiek-poort  alleen-kijken-pagina (bord + uitzending) voor internet; 0 = uit\n  \
                     fetch            laat zien wat Claude van de website leest\n  \
                     scenes           schrijf voorbeelden van de uitzendbeelden naar data/scenes"
                );
                std::process::exit(0);
            }
            other => opts.command = Some(other.to_string()),
        }
    }
    opts
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
        let ex = extract::extract_liturgy(&opts.liturgy_url).await?;
        println!("{ex:#?}");
        return Ok(());
    }

    if opts.command.as_deref() == Some("scenes") {
        return render::write_examples(&opts.data_dir.join("scenes"));
    }

    let show = show::Show::new(opts.data_dir.join("liturgie.json"), opts.liturgy_url.clone()).await;
    let engine = stream::Engine::new()?;
    let director = director::Director::new(engine.clone(), &opts.data_dir);
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
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("let op: ANTHROPIC_API_KEY is niet gezet; liturgie ophalen van de website werkt dan niet");
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
