use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Copy prompt files to the output directory so they live next to the binary.
    let out_dir = env::var("OUT_DIR").unwrap();
    // OUT_DIR is something like target/debug/build/zbobr-xxx/out
    // The binary lands in target/debug/, so walk up 3 levels.
    let target_dir = Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("Cannot find target dir from OUT_DIR");

    let prompts_dest = target_dir.join("prompts");
    fs::create_dir_all(&prompts_dest).expect("Failed to create prompts dir in target");

    for name in &["planner.md", "worker.md"] {
        let src = Path::new("prompts").join(name);
        let dst = prompts_dest.join(name);
        fs::copy(&src, &dst).unwrap_or_else(|e| {
            panic!("Failed to copy {} to {}: {e}", src.display(), dst.display())
        });
    }

    // Re-run if prompt files change
    println!("cargo:rerun-if-changed=prompts/planner.md");
    println!("cargo:rerun-if-changed=prompts/worker.md");
}
