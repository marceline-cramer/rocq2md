# rocq2md

A command-line tool written in Rust that converts Rocq files (`.v`) to Markdown documents (`.md`).

## Description

`rocq2md` converts Rocq files to Markdown by transforming block comments to text and Rocq code to Markdown code blocks.

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.86.0 or later.
- Cargo (comes with Rust).

### Clone the repo and build the package

```bash
git clone https://github.com/matematiflo/rocq2md.git
cd rocq2md
cargo build
```

## Running the program

The program can be run directly after cloning (no build necessary) or as a local or global executable.

### Directly after cloning

The program can be run directly using `cargo run -- <arguments>`. A test file `sample.v` is provided.

```bash
cargo run -- test_files/sample.v
```

### Local usage (inside the repo)

After building an executable, you can run it as follows. Use either

```bash
cargo build
./target/debug/rocq2md <arguments> 
```

or (for an optimized executable)

```bash
cargo build --release
./target/release/rocq2md <arguments>
```

### Adding the executable to your Rust environment

If you want to add `rocq2md` to your Rust environment, install the package globally as follows.

```bash
cargo install --path .
```

Check installation:

```bash
rocq2md --version
```

Check location (should be `~/.cargo/bin`):

```bash
which rocq2md
```

Alternatively, you can [use cargo](#rust-executable) to download and install the executable directly from [crates.io](https://lib.rs/crates/rocq2md).

## Commands

`rocq2md` can convert single files or all Rocq files in a specified source directory (including its subfolders). For single files, you can specify the target (location and name). For directories, you can specify the target directory (which is created automatically if not already present).

```bash
rocq2md <file.v>                # converts <file.v> to <file.md>
rocq2md <src_dir>               # converts all Rocq files in <src_dir> to Markdown files
rocq2md <file.v> <new_name.md>  # converts <file.v> to <new_name.md> (subfolders allowed)
rocq2md <src_dir> <tgt_dir>     # converts all Rocq files from <src_dir>; puts them in <tgt_dir>
```

Examples you can run in this repo:

```bash
rocq2md test_files/sample.v
rocq2md test_files/sample.v converted_files/sample.md
rocq2md test_files
rocq2md test_files converted_files
```

## Features

- Converts top-level Rocq comments `(* ... *)` to Markdown text.
- Puts Rocq code inside ` ```rocq ` code fences.
- Preserves directory structure from source to target.
- Nested folders are also processed.
- Any text enclosed between `--#--` symbols is removed from the generated markdown file.
- A `(* ... *)` comment is converted to Markdown only if the `(*` appears strictly at the beginning of a line (column 0) and the comment is not contained inside a proof environment.
- In nested comments `(* (* ... *) ... *)`, only the outermost delimiters are stripped.
- Comments that are not converted generate a warning during conversion. The reason for the warning is logged into the `rocq2md.log` file (in the absence of any warning, no log file is created).
- Comments contained in a proof (are not converted and) do not generate a warning. The total number of warnings is printed to the console after conversion.

## Examples

### Basic usage

````rocq
(* # Introduction

This module defines a trivial lemma. *)

Definition foo := 1. (* not converted: this comment does not start a line *)

 (* not converted: does not start at columnn 0 *)

Lemma foo_eq : foo = 1.
Proof.
  (* This comment is inside a proof: kept as code, no warning. *)
  reflexivity.
Qed.
````

produces:

````markdown
# Introduction

This module defines a trivial lemma.

```rocq
Definition foo := 1. (* not converted: is not located at the start of a line *)

  (* not converted: does not start at columnn 0 *)

Lemma foo_eq : foo = 1.
Proof.
  (* This comment is inside a proof: kept as code, no warning. *)
  reflexivity.
Qed.
```
````

### Ignored text

Text placed between two `--#--` markers inside a converted comment block is removed from the Markdown output, along with the markers themselves. This works both inline (on a single line) and as a block spanning multiple lines:

```rocq
(* Keep this. --#-- Remove this internal note. --#-- Keep this too.

--#--
This whole paragraph is removed as well.
--#--

Kept again. *)
```

## Package information

### Files

- `src/rocq2md_core.rs`: Parsing and converting rules.
- `src/lib.rs`: Library interface that exports public functions.
- `src/main.rs`: Command-line interface.

### Rust executable

You can install `rocq2md` directly from [crates.io](https://lib.rs/crates/rocq2md):

```bash
cargo install rocq2md
```

This will download, compile, and install the package in your `~/.cargo/bin` directory, making it available in your PATH.

Alternatively, you can [install your locally built version](#adding-the-executable-to-your-rust-environment):

```bash
cargo install --path .
```

### Testing

Unit tests are located in `src/rocq2md_core.rs` and cover comment conversion, column-0 detection, proof-environment detection, nested comments, and the `--#--` marker.

```bash
# Run all tests
cargo test
```

### Acknowledgments

This project follows the design of the following projects, adapting it from Lean to Rocq:

- [arthurpaulino/lean2md](https://github.com/arthurpaulino/lean2md), the original Lean to Markdown conversion tool, written in Python.
- [Seasawher/mdgen](https://github.com/Seasawher/mdgen), based on `lean2md` and written in Lean (to generate documentation directly inside a Lean project).
- [fosterfarrell9/lean2md](https://github.com/fosterfarrell9/lean2md), based on `mdgen` and written in Rust.

### License

This project is licensed under the MIT License.

### Contributing

Contributions are welcome. Feel free to open an issue or submit a pull request.
