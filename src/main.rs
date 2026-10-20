//! Stream & Beam shows the liturgy of HHG Tholen (psalms, readings, sermon theme) on the
//! screens in church. The TVs open http://<this-pc>:8080/beam and follow the operator,
//! who works from http://<this-pc>:8080.

mod books;
mod claude;
mod director;
mod extract;
mod render;
mod show;
mod stream;
mod texts;
mod web;

use std::{path::PathBuf, sync::Arc};

struct Options {
    port: u16,
    data_dir: PathBuf,
    old_spelling: bool,
    liturgy_url: String,
    command: Option<String>,
}

fn parse_args() -> Options {
    let mut opts = Options {
        port: 8080,
        data_dir: "data".into(),
        old_spelling: false,
        liturgy_url: extract::LITURGY_URL.into(),
        command: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--poort" | "--port" => opts.port = args.next().and_then(|p| p.parse().ok()).unwrap_or(8080),
            "--data" => opts.data_dir = args.next().unwrap_or_default().into(),
            "--oude-spelling" => opts.old_spelling = true,
            "--liturgie-url" => opts.liturgy_url = args.next().unwrap_or_default(),
            "-h" | "--help" => {
                println!(
                    "streamandbeam [--poort 8080] [--data data] [--oude-spelling] [--liturgie-url URL] [fetch|scenes]\n\n  \
                     --oude-spelling  psalmen in de spelling van 1774 i.p.v. door Claude gemoderniseerd\n  \
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
        println!("{}", serde_json::to_string_pretty(&ex)?);
        return Ok(());
    }

    if opts.command.as_deref() == Some("scenes") {
        return render::write_examples(&opts.data_dir.join("scenes"));
    }

    let texts = Arc::new(texts::Texts { data_dir: opts.data_dir.clone(), modernize: !opts.old_spelling });
    let show = show::Show::new(opts.data_dir.join("liturgie.json"), texts, opts.liturgy_url.clone()).await;
    let engine = stream::Engine::new()?;
    let director = director::Director::new(engine, opts.data_dir.join("instellingen.json"));
    let photo = std::fs::read(opts.data_dir.join("kerk.jpg"))
        .ok()
        .or_else(|| Some(include_bytes!("../assets/images/kerk.jpg").to_vec()));
    tokio::spawn(director.clone().run(show.clone(), photo));
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("let op: ANTHROPIC_API_KEY is niet gezet; liturgie ophalen van de website werkt dan niet");
    }

    let base = format!("http://{}:{}", lan_ip(), opts.port);
    println!("Bediening:          {base}");
    println!("Schermen (TV's):    {base}/beam");
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;
    axum::serve(listener, web::router(show, director, format!("{base}/beam"))).await?;
    Ok(())
}
