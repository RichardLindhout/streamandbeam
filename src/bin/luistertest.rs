//! Luistertest: transcribes a piece of a recording with a Whisper model, with and without
//! the church vocabulary prompt, and reports how long it took. For choosing a model.
//!
//!     target/release/luistertest modellen/ggml-small-q5_1.bin testdata/preek-geluid.m4a [start] [seconden]

use std::process::Command;
use std::time::Instant;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

/// Words that occur in every service; telling Whisper beforehand makes it hear them better.
const PROMPT: &str = "Kerkdienst van de Hersteld Hervormde Gemeente. Laten wij zingen Psalm 42 vers 5. \
Wij lezen uit Johannes. Laten wij bidden. De Heere, de gemeente, de Heidelbergse Catechismus, Zondag 22, \
de Schriftlezing, het gebed, de collecte, amen.";

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (model, file) = (&args[1], &args[2]);
    let start = args.get(3).map_or("0", String::as_str);
    let secs = args.get(4).map_or("30", String::as_str);
    let pcm = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-ss", start, "-t", secs, "-i", file, "-ac", "1", "-ar", "16000", "-f", "f32le", "-"])
        .output()?
        .stdout;
    let samples: Vec<f32> = pcm.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let ctx = WhisperContext::new_with_params(model, Default::default())?;
    for prompt in [None, Some(PROMPT)] {
        let mut state = ctx.create_state()?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("nl"));
        params.set_n_threads(4);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        if let Some(p) = prompt {
            params.set_initial_prompt(p);
        }
        let t = Instant::now();
        state.full(params, &samples)?;
        let text: String = state.as_iter().map(|s| s.to_string()).collect::<Vec<_>>().join(" ");
        println!(
            "\n== {} {} — {:.1} s rekenen voor {secs} s geluid\n{}",
            model.rsplit('/').next().unwrap_or(model),
            if prompt.is_some() { "met woordenlijst" } else { "zonder woordenlijst" },
            t.elapsed().as_secs_f64(),
            text.trim()
        );
    }
    Ok(())
}
