use std::path::PathBuf;
use clap::Parser;

#[derive(Parser)]
struct Args {
    input: PathBuf,
    output: PathBuf,

    #[arg(long)]
    name: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let bf = std::fs::read_to_string(&args.input)?;
    let name = args.name.unwrap_or_else(|| args.input.file_name().unwrap().to_string_lossy().into_owned());
    let wasm = brainwasm::bf_to_wasm(&bf, Some(&name), 8, Some((CUSTOM_NAME.to_owned(), CUSTOM_DATA.to_owned().into_bytes())))?;
    std::fs::write(args.output, wasm)?;
    Ok(())
}

const CUSTOM_NAME: &str = "b2w";
const CUSTOM_DATA: &str =
    "this wasm file was created using `b2w`, a program for converting brainf**k to wasm.";
