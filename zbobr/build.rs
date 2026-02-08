use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // OUT_DIR is something like target/debug/build/zbobr-xxx/out
    // The binary lands in target/debug/, so walk up 3 levels.
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("Cannot find target dir from OUT_DIR");

    // Copy workflow prompt files next to the binary
    copy_dir("resources/prompts", &target_dir.join("prompts"), &[
        "planner-workflow.md",
        "worker-workflow.md",
    ]);

    // Copy resource files next to the binary
    copy_dir("resources", &target_dir.join("resources"), &[
        "README.md",
        "zbobr.env",
        "run.sh",
        "run.cmd",
    ]);

    // Copy all prompts directory to resources for setup
    copy_dir_recursive("resources/prompts", &target_dir.join("resources/prompts"));
}

fn copy_dir(src_dir: &str, dest_dir: &Path, files: &[&str]) {
    fs::create_dir_all(dest_dir).unwrap_or_else(|e| {
        panic!("Failed to create {}: {e}", dest_dir.display())
    });

    for name in files {
        let src = Path::new(src_dir).join(name);
        let dst = dest_dir.join(name);
        fs::copy(&src, &dst).unwrap_or_else(|e| {
            panic!("Failed to copy {} to {}: {e}", src.display(), dst.display())
        });
        println!("cargo:rerun-if-changed={}/{}", src_dir, name);
    }
}

fn copy_dir_recursive(src_dir: &str, dest_dir: &Path) {
    fs::create_dir_all(dest_dir).unwrap_or_else(|e| {
        panic!("Failed to create {}: {e}", dest_dir.display())
    });

    let entries = fs::read_dir(src_dir).unwrap_or_else(|e| {
        panic!("Failed to read directory {}: {e}", src_dir)
    });

    for entry in entries {
        let entry = entry.unwrap();
        let src = entry.path();
        let file_name = entry.file_name();
        let dst = dest_dir.join(&file_name);

        if src.is_file() {
            fs::copy(&src, &dst).unwrap_or_else(|e| {
                panic!("Failed to copy {} to {}: {e}", src.display(), dst.display())
            });
            println!("cargo:rerun-if-changed={}", src.display());
        }
    }
}
