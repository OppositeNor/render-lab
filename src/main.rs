//! Render Lab: illustrated investigations of game-engine features, rendered
//! with wgpu and WGSL. The lab (lab/) is a library this binary and every
//! package's library link; the investigations come from the packages in
//! packages/, whose libraries the lab opens at run time.

fn main() {
    if let Err(e) = lab::cli::main(lab::cli::App { title: "o3de/lighting" }) {
        // As a `main` that returns anyhow's Result prints it.
        eprintln!("Error: {e:?}");
        std::process::exit(1);
    }
}
