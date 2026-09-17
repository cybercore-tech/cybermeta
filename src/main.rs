use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use cybermeta::meta::{list_presets, ExportFormat, MetaDoc, Preset};
use cybermeta::tui;

#[derive(Debug, Parser)]
#[command(
    name = "cybermeta",
    about = "TUI-first EXIF workstation for the Cybercore Systems Framework",
    version
)]
struct Cli {
    /// Image path — opens the TUI when no subcommand is given
    file: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List tags (JSON by default)
    Show {
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = OutFormat::Json)]
        format: OutFormat,
    },
    /// Export tags to stdout
    Export {
        file: PathBuf,
        #[arg(short = 'f', long, value_enum, default_value_t = OutFormat::Json)]
        format: OutFormat,
    },
    /// Strip GPS and/or all non-structural metadata
    Strip {
        file: PathBuf,
        #[arg(long, conflicts_with = "all")]
        gps: bool,
        #[arg(long)]
        all: bool,
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
        #[arg(long)]
        in_place: bool,
    },
    /// Set a string tag: TAG=VALUE
    Set {
        file: PathBuf,
        assignment: String,
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
        #[arg(long)]
        in_place: bool,
    },
    /// Apply a named scrub/spoof preset
    Spoof {
        file: Option<PathBuf>,
        #[arg(long, required_unless_present = "list")]
        preset: Option<String>,
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
        #[arg(long)]
        in_place: bool,
        /// List available presets and exit
        #[arg(long)]
        list: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutFormat {
    Json,
    Tsv,
}

impl From<OutFormat> for ExportFormat {
    fn from(value: OutFormat) -> Self {
        match value {
            OutFormat::Json => ExportFormat::Json,
            OutFormat::Tsv => ExportFormat::Tsv,
        }
    }
}

fn main() -> ExitCode {
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("cybermeta: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            let path = cli.file;
            if path.is_none() && !io::stdout().is_terminal() {
                bail!("no TTY and no subcommand — use show/export/strip/set/spoof");
            }
            tui::run(path)
        }
        Some(Commands::Show { file, format }) => {
            let doc = MetaDoc::open(&file)?;
            print!("{}", doc.export_string(format.into())?);
            Ok(())
        }
        Some(Commands::Export { file, format }) => {
            let doc = MetaDoc::open(&file)?;
            print!("{}", doc.export_string(format.into())?);
            Ok(())
        }
        Some(Commands::Strip {
            file,
            gps,
            all,
            output,
            in_place,
        }) => {
            if !gps && !all {
                bail!("specify --gps and/or --all");
            }
            let mut doc = MetaDoc::open(&file)?;
            if gps {
                let n = doc.strip_gps();
                eprintln!("stripped {n} GPS-related tags");
            }
            if all {
                let n = doc.strip_all();
                eprintln!("stripped {n} non-structural tags");
            }
            let dest = doc.save(output.as_deref(), in_place)?;
            eprintln!("wrote {}", dest.display());
            Ok(())
        }
        Some(Commands::Set {
            file,
            assignment,
            output,
            in_place,
        }) => {
            let mut doc = MetaDoc::open(&file)?;
            doc.set_tag_from_assignment(&assignment)?;
            let dest = doc.save(output.as_deref(), in_place)?;
            eprintln!("wrote {}", dest.display());
            Ok(())
        }
        Some(Commands::Spoof {
            file,
            preset,
            output,
            in_place,
            list,
        }) => {
            if list {
                for p in list_presets() {
                    println!("{}\t{}", p.as_str(), p.description());
                }
                return Ok(());
            }
            let file = file.context("FILE is required unless --list")?;
            let preset_name = preset.context("--preset is required unless --list")?;
            let Some(preset) = Preset::parse(&preset_name) else {
                bail!(
                    "unknown preset `{preset_name}` — try: {}",
                    list_presets()
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            };
            let mut doc =
                MetaDoc::open(&file).with_context(|| format!("open {}", file.display()))?;
            doc.apply_preset(preset)?;
            let dest = doc.save(output.as_deref(), in_place)?;
            eprintln!("applied {} → {}", preset.as_str(), dest.display());
            Ok(())
        }
    }
}
