//! # momoi-ctl
//!
//! CLI control tool for the momoi daemon.
//!
//! ## Usage
//! ```text
//! momoi-ctl reload
//! momoi-ctl status
//! momoi-ctl outputs
//! momoi-ctl set <OUTPUT> <WALLPAPER>
//! momoi-ctl quit
//! ```

use anyhow::{Context, Result, bail};
use ipc_protocol::{Command, Response, socket_path};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = parse_args(&args)?;
    let response = send_command(cmd).await?;
    print_response(response);
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Command> {
    match args.first().map(String::as_str) {
        Some("reload") => Ok(Command::Reload),
        Some("status") => Ok(Command::Status),
        Some("outputs") => Ok(Command::ListOutputs),
        Some("quit") => Ok(Command::Quit),
        Some("set") => {
            let output = args
                .get(1)
                .context("set requires <OUTPUT> argument")?
                .clone();
            let wallpaper = args
                .get(2)
                .context("set requires <WALLPAPER> argument")?
                .clone();
            Ok(Command::SetWallpaper { output, wallpaper })
        }
        Some(unknown) => bail!(
            "unknown command '{unknown}'.\n\nUsage:\n  momoi-ctl reload\n  momoi-ctl status\n  momoi-ctl outputs\n  momoi-ctl set <OUTPUT> <WALLPAPER>\n  momoi-ctl quit"
        ),
        None => bail!(
            "no command given.\n\nUsage:\n  momoi-ctl reload\n  momoi-ctl status\n  momoi-ctl outputs\n  momoi-ctl set <OUTPUT> <WALLPAPER>\n  momoi-ctl quit"
        ),
    }
}

async fn send_command(cmd: Command) -> Result<Response> {
    let path = socket_path();
    let stream = UnixStream::connect(&path)
        .await
        .with_context(|| format!("could not connect to momoi socket at {}", path.display()))?;

    let (reader, mut writer) = stream.into_split();

    // Send command as a single JSON line.
    let mut line = serde_json::to_string(&cmd)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;
    writer.shutdown().await?;

    // Read single-line JSON response.
    let mut resp_line = String::new();
    BufReader::new(reader).read_line(&mut resp_line).await?;

    let response: Response =
        serde_json::from_str(resp_line.trim()).context("failed to parse daemon response")?;
    Ok(response)
}

fn print_response(resp: Response) {
    match resp {
        Response::Ok { message } => {
            println!(
                "ok{}",
                message.map(|m| format!(": {m}")).unwrap_or_default()
            );
        }
        Response::Error { message } => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
        Response::Status(s) => {
            println!("momoi v{}", s.version);
            println!("  outputs   : {}", s.active_outputs);
            println!("  frames    : {}", s.total_frames);
            println!(
                "  audio     : {}",
                if s.audio_active { "active" } else { "inactive" }
            );
        }
        Response::Outputs(outputs) => {
            if outputs.is_empty() {
                println!("no outputs");
                return;
            }
            for o in outputs {
                println!(
                    "{:<12} {}  {:<30}  {:.1} fps",
                    o.name, o.resolution, o.wallpaper, o.fps
                );
            }
        }
    }
}
