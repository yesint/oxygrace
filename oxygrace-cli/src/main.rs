//! `oxygrace` CLI: render a Grace `.agr`/`.xvg` file to a PNG or SVG.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

/// Render a Grace `.agr`/`.xvg` file to a PNG or SVG image.
#[derive(Parser, Debug)]
#[command(name = "oxygrace", version, about)]
struct Cli {
    /// Input `.agr`/`.xvg` file.
    input: PathBuf,

    /// Output path (defaults to the input name with a `.png` extension).
    /// A `.svg` extension selects SVG output.
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

    let output = cli.output.unwrap_or_else(|| cli.input.with_extension("png"));
    let ext_is = |e: &str| output.extension().is_some_and(|x| x.eq_ignore_ascii_case(e));
    // A project extension converts between formats instead of rendering.
    if ext_is("agr") || ext_is("oxgr") {
        oxygrace::save(&project, &output)
            .with_context(|| format!("writing {}", output.display()))?;
        eprintln!("wrote {}", output.display());
        return Ok(());
    }
    let bytes = if ext_is("svg") {
        oxygrace::render_svg(&project).into_bytes()
    } else {
        oxygrace::render_png(&project)
    };
    std::fs::write(&output, bytes).with_context(|| format!("writing {}", output.display()))?;
    eprintln!("wrote {}", output.display());
    Ok(())
}
