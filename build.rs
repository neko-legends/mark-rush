use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct BuildTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

struct GitInfo {
    commit: String,
    dirty: bool,
}

struct BuildVersion {
    version: String,
    label: String,
    metadata: String,
    time: BuildTime,
}

fn main() {
    watch_build_inputs();

    let version = build_version();
    println!("cargo:rustc-env=MARKRUSH_BUILD_VERSION={}", version.version);
    println!("cargo:rustc-env=MARKRUSH_BUILD_LABEL={}", version.label);

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let resource_path = write_windows_resource(&version);
        embed_resource::compile(resource_path, embed_resource::NONE)
            .manifest_optional()
            .expect("failed to embed Windows app resources");
    }
}

fn watch_build_inputs() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=README.md");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=docs");
    println!("cargo:rerun-if-changed=assets/fonts");
    println!("cargo:rerun-if-changed=assets/icons/MarkRush.ico");
    println!("cargo:rerun-if-changed=assets/icons/MarkRush-256.png");

    if let Some(git_dir) = git_path(["rev-parse", "--git-dir"]) {
        let git_dir = absolutize(&git_dir);
        println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
        println!("cargo:rerun-if-changed={}", git_dir.join("index").display());

        if let Some(ref_name) = git_text(["symbolic-ref", "-q", "HEAD"]) {
            println!(
                "cargo:rerun-if-changed={}",
                git_dir.join(ref_name).display()
            );
        }
    }
}

fn build_version() -> BuildVersion {
    let time = current_build_time();
    let git = git_info();
    let metadata = if git.dirty {
        format!("g{}.dirty", git.commit)
    } else {
        format!("g{}", git.commit)
    };
    let patch = format!(
        "{}{:02}{:02}{:02}",
        time.day, time.hour, time.minute, time.second
    );
    let date = format!("{:04}-{:02}-{:02}", time.year, time.month, time.day);
    let version = format!("{}.{}.{}+{}", time.year, time.month, patch, metadata);
    let label = format!("ver {date} - {metadata}");

    BuildVersion {
        version,
        label,
        metadata,
        time,
    }
}

fn git_info() -> GitInfo {
    let root = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_owned());
    let top_level = git_text(["rev-parse", "--show-toplevel"]);
    let in_repo = top_level
        .as_deref()
        .map(|value| normalize_path(value) == normalize_path(&root))
        .unwrap_or(false);

    if !in_repo {
        return GitInfo {
            commit: "nogit".to_owned(),
            dirty: false,
        };
    }

    let commit = git_text(["rev-parse", "--short=8", "HEAD"]).unwrap_or_else(|| "nogit".to_owned());
    let dirty = !git_success(["diff-index", "--quiet", "HEAD", "--"]);

    GitInfo { commit, dirty }
}

fn current_build_time() -> BuildTime {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400) as u32;
    let (year, month, day) = civil_from_days(days);

    BuildTime {
        year,
        month,
        day,
        hour: seconds_of_day / 3_600,
        minute: (seconds_of_day % 3_600) / 60,
        second: seconds_of_day % 60,
    }
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let days = days_since_unix_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };

    (year as i32, month as u32, day as u32)
}

fn write_windows_resource(version: &BuildVersion) -> PathBuf {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let icon_path = manifest_dir.join("assets/icons/MarkRush.ico");
    let rc_path = out_dir.join("MarkRush.rc");
    let resource = windows_resource_text(version, &icon_path);
    fs::write(&rc_path, resource).expect("failed to write generated Windows resource");
    rc_path
}

fn windows_resource_text(version: &BuildVersion, icon_path: &Path) -> String {
    let numeric_version = format!(
        "{},{},{},{}",
        version.time.year,
        version.time.month,
        version.time.day,
        version.time.hour * 100 + version.time.minute
    );
    let icon_path = rc_escape(&icon_path.display().to_string());
    let file_version = rc_escape(&version.version);
    let comments = rc_escape(&format!("Build metadata: {}", version.metadata));

    format!(
        r#"1 ICON "{icon_path}"

1 VERSIONINFO
FILEVERSION {numeric_version}
PRODUCTVERSION {numeric_version}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "flashosophy\0"
            VALUE "FileDescription", "Fast local Markdown viewer and editor\0"
            VALUE "FileVersion", "{file_version}\0"
            VALUE "InternalName", "markrush\0"
            VALUE "OriginalFilename", "markrush.exe\0"
            VALUE "ProductName", "MarkRush\0"
            VALUE "ProductVersion", "{file_version}\0"
            VALUE "Comments", "{comments}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
    )
}

fn rc_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn git_text<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = git_output(args)?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if text.is_empty() { None } else { Some(text) }
    } else {
        None
    }
}

fn git_path<const N: usize>(args: [&str; N]) -> Option<PathBuf> {
    git_text(args).map(PathBuf::from)
}

fn git_success<const N: usize>(args: [&str; N]) -> bool {
    git_output(args)
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<std::process::Output> {
    let root = env::var("CARGO_MANIFEST_DIR").ok()?;
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
}

fn normalize_path(value: &str) -> String {
    absolutize(&PathBuf::from(value))
        .display()
        .to_string()
        .replace('\\', "/")
        .to_lowercase()
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_owned())).join(path)
    }
}
