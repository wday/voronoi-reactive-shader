//! spout-publish — pipe raw RGBA frames to a Spout sender
//!
//! Usage:
//!   ffmpeg ... -f rawvideo -pix_fmt rgba -s WxH pipe:1 | spout-publish --name "MySource" --width W --height H
//!
//! Reads raw RGBA frames (W * H * 4 bytes each) from stdin and publishes
//! each frame via Spout's SendImage API. Any Spout receiver (Resolume,
//! TouchDesigner, etc.) can pick up the source by name.
//!
//! Requires SpoutLibrary.dll in PATH or next to the executable.

use std::io::{self, Read};
use std::process;

mod spout_ffi;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut name = String::from("spout-publish");
    let mut width: u32 = 0;
    let mut height: u32 = 0;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--name" | "-n" => {
                i += 1;
                name = args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("error: --name requires a value");
                    process::exit(1);
                });
            }
            "--width" | "-w" => {
                i += 1;
                width = args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                    eprintln!("error: --width requires a numeric value");
                    process::exit(1);
                });
            }
            "--height" | "-h" => {
                i += 1;
                height = args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                    eprintln!("error: --height requires a numeric value");
                    process::exit(1);
                });
            }
            "--help" => {
                eprintln!("spout-publish — pipe raw RGBA frames to a Spout sender");
                eprintln!();
                eprintln!("Usage:");
                eprintln!("  ffmpeg ... -f rawvideo -pix_fmt rgba -s WxH pipe:1 | \\");
                eprintln!("    spout-publish --name NAME --width W --height H");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  -n, --name    Spout sender name (default: spout-publish)");
                eprintln!("  -w, --width   Frame width in pixels");
                eprintln!("  -h, --height  Frame height in pixels");
                process::exit(0);
            }
            other => {
                eprintln!("error: unknown argument: {other}");
                process::exit(1);
            }
        }
        i += 1;
    }

    if width == 0 || height == 0 {
        eprintln!("error: --width and --height are required");
        process::exit(1);
    }

    let frame_bytes = (width * height * 4) as usize;
    eprintln!(
        "spout-publish: sending {width}x{height} RGBA as \"{name}\" ({} bytes/frame)",
        frame_bytes
    );

    let sender = match spout_ffi::SpoutSender::new(&name, width, height) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to create Spout sender: {e}");
            process::exit(1);
        }
    };

    let mut buf = vec![0u8; frame_bytes];
    let mut stdin = io::stdin().lock();
    let mut frame_count: u64 = 0;

    loop {
        match stdin.read_exact(&mut buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                eprintln!("spout-publish: stdin closed after {frame_count} frames");
                break;
            }
            Err(e) => {
                eprintln!("spout-publish: read error: {e}");
                process::exit(1);
            }
        }

        sender.send_image(&buf, width, height);
        frame_count += 1;
    }
}
