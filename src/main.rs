use rocq2md::{process_directory, process_file};
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        if args[1] == "--version" {
            let version = env!("CARGO_PKG_VERSION");
            println!("rocq2md version {}", version);
            return Ok(());
        } else {
            let src = PathBuf::from(&args[1]);

            if src.is_file() && src.extension().is_some_and(|ext| ext == "v") {
                // Case: rocq2md <file.v>
                let tgt = src.with_extension("md");
                return process_file(&src, &tgt);
            } else if src.is_dir() {
                // Case: rocq2md <rocq_src_dir> (converts in place)
                return process_directory(&src, &src);
            }
        }
    } else if args.len() == 3 {
        let src = PathBuf::from(&args[1]);
        let tgt = PathBuf::from(&args[2]);

        if src.is_file() {
            // Case: rocq2md <rocq_src_file> <md_tgt_file>
            return process_file(&src, &tgt);
        } else if src.is_dir() {
            // Case: rocq2md <rocq_src_dir> <md_tgt_dir>
            return process_directory(&src, &tgt);
        }
    }

    // If we reach here, arguments were invalid
    println!("Error: Invalid arguments or file(s) not found.");
    println!("Usage: Run one of the following commands.");
    println!("  rocq2md <file.v>                     # converts <file.v> to <file.md>");
    println!("  rocq2md <src_dir>                    # converts all Rocq files in <src_dir> to Markdown files");
    println!("  rocq2md <file.v> <new_name.md>       # converts <file.v> to <new_name.md> (subfolders allowed)");
    println!("  rocq2md <rocq_src_dir> <md_tgt_dir>  # converts all Rocq files from <src_dir>; puts them in <tgt_dir>");
    Ok(())
}
