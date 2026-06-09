//! `oxygrace` CLI: render a Grace `.agr`/`.xvg` file to a PNG.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

/// Render a Grace `.agr`/`.xvg` file to a PNG image.
#[derive(Parser, Debug)]
#[command(name = "oxygrace", version, about)]
struct Cli {
    /// Input `.agr`/`.xvg` file.
    input: PathBuf,

    /// Output PNG path (defaults to the input name with a `.png` extension).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Override page width in pixels.
    #[arg(long)]
    width: Option<u32>,

    /// Override page height in pixels.
    #[arg(long)]
    height: Option<u32>,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let cli = Cli::parse();

    let mut project = oxygrace::load(&cli.input)
        .with_context(|| format!("reading {}", cli.input.display()))?;
    if let Some(w) = cli.width {
        project.page_width = w;
    }
    if let Some(h) = cli.height {
        project.page_height = h;
    }

    let png = oxygrace::render_png(&project);

    let output = cli.output.unwrap_or_else(|| cli.input.with_extension("png"));
    std::fs::write(&output, png).with_context(|| format!("writing {}", output.display()))?;
    eprintln!("wrote {}", output.display());
    Ok(())
}
