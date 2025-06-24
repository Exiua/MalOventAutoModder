use std::path::PathBuf;

use clap::Parser;
use malovent_automodder::modder::mod_file;

#[derive(Parser, Debug)]
#[command(version = "v1.0.0", about = "Auto mods MavOlent's script.rpy file with walkthrough features", long_about = None)]
struct Args {
    #[arg(help="Path to the script.rpy file to be modified")]
    path: PathBuf,
}

fn main() {
    let args = Args::parse();
    let file_path = args.path;

    if !file_path.exists() {
        eprintln!("Error: The specified file does not exist: {:?}", file_path);
        std::process::exit(1);
    }

    let file_name = file_path.file_name()
        .and_then(|name| name.to_str());

    if let Some(name) = file_name {
        if name != "script.rpy" {
            eprintln!("Error: The specified file is not script.rpy: {:?}", file_name);
            std::process::exit(1);
        }
    }
    else {
        eprintln!("Error: The specified file does not have a valid name.");
        std::process::exit(1);
    }

    match mod_file(file_path.to_str().unwrap()) {
        Ok(_) => println!("File modified successfully."),
        Err(e) => eprintln!("Error modifying file: {}", e),
    }
}
