//! `mutastic` is one command for both editions: in a Cargo project it runs itself, and in a Maven or
//! Gradle project it hands the command, unchanged, to the Java edition, which it fetches the first
//! time one is needed. These run the built binary with a stand-in Java edition, a shell script that
//! writes down what it was given, and a release directory on disk: never the network.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::Digest;

/// A new, empty directory of the test's own.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("mutastic-front-{}", std::process::id()))
        .join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A stand-in for the Java edition: it writes its arguments, one a line, where `RECORD` says, and
/// exits with `FAKE_EXIT`.
const ENGINE: &str = "#!/bin/sh\n\
    if [ \"$1\" = --version ]; then echo 'mutastic 9.9.9'; exit 0; fi\n\
    printf '%s\\n' \"$@\" > \"$RECORD\"\n\
    exit \"${FAKE_EXIT:-0}\"\n";

/// The binary, with none of the settings a person's environment might hold.
fn mutastic(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutastic"));
    command
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_JAVA_ENGINE")
        .env_remove("MUTASTIC_VERSION")
        .env_remove("MUTASTIC_JAVA_RELEASES")
        .env("MUTASTIC_HOME", dir.join("home"))
        .env("NO_COLOR", "1");
    command
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// What the stand-in was given, or `None` when it never ran.
fn recorded(record: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(record).ok()?;
    Some(text.lines().map(str::to_string).collect())
}

/// Runs `args` in `dir` with the stand-in Java edition, exiting with `exit`.
fn forwarded(root: &Path, dir: &Path, args: &[&str], exit: u8) -> (Output, Option<Vec<String>>) {
    let engine = root.join("engine/bin/mutastic");
    executable(&engine, ENGINE);
    let record = root.join("record");
    std::fs::remove_file(&record).ok();
    let output = mutastic(dir, args)
        .env("MUTASTIC_JAVA_ENGINE", &engine)
        .env("RECORD", &record)
        .env("FAKE_EXIT", exit.to_string())
        .output()
        .unwrap();
    (output, recorded(&record))
}

#[test]
fn a_java_project_gets_the_java_edition_with_every_argument_and_exit_status() {
    let root = scratch("forward");
    let maven = root.join("shop");
    write(&maven.join("pom.xml"), "<project/>");
    let gradle = root.join("game");
    write(&gradle.join("settings.gradle.kts"), "");
    let maven_path = maven.display().to_string();
    let cases: &[(&Path, &[&str], u8)] = &[
        (&root, &["gate", "check", &maven_path, "--hook=claude"], 0),
        (
            &root,
            &["run", &maven_path, ":app", "--since=main", "--threshold=70"],
            1,
        ),
        (&root, &["verify", "m855c8e3c,m1234abcd", &maven_path], 3),
        (&root, &["explain", "m855c8e3c", &maven_path], 4),
        (
            &root,
            &["ignore", "m855c8e3c", &maven_path, "--reason=a log line"],
            2,
        ),
        // A command only the Java edition has goes through too, from the project's own directory.
        (&gradle, &["next", "--top=3"], 0),
        (&gradle, &["run", "--help"], 0),
    ];
    for (dir, args, exit) in cases {
        let (output, given) = forwarded(&root, dir, args, *exit);
        assert_eq!(
            output.status.code(),
            Some(i32::from(*exit)),
            "{args:?}: {}",
            text(&output.stderr)
        );
        assert_eq!(
            given.as_deref(),
            Some(
                args.iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .as_slice()
            )
        );
    }
    // --edition is the front door's: the Java edition never sees it.
    let (output, given) = forwarded(&root, &maven, &["run", "--edition=java"], 0);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(given.unwrap(), ["run"]);
}

#[test]
fn a_rust_project_or_none_never_reaches_the_java_edition() {
    let root = scratch("rust");
    let crate_dir = root.join("crate");
    write(&crate_dir.join("Cargo.toml"), "");
    let nothing = root.join("nothing");
    std::fs::create_dir_all(&nothing).unwrap();
    for dir in [&crate_dir, &nothing] {
        // The plugins' gate in a project mutastic can't read: it says nothing, and lets the agent go.
        let (output, given) = forwarded(&root, dir, &["gate", "open", ".", "--hook=claude"], 9);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        assert_eq!(given, None, "{}", dir.display());
        assert!(!root.join("crate/home").exists() && !root.join("nothing/home").exists());
    }
    // --edition=rust wins over the build file.
    let maven = root.join("shop");
    write(&maven.join("pom.xml"), "<project/>");
    let (output, given) = forwarded(
        &root,
        &maven,
        &["gate", "open", "--edition=rust", "--hook=claude"],
        9,
    );
    assert_eq!((output.status.code(), given), (Some(0), None));
}

#[test]
fn a_mixed_tree_runs_the_edition_of_the_directory_pointed_at() {
    let root = scratch("mixed");
    let game = root.join("tictactoe");
    write(&game.join("settings.gradle.kts"), "");
    write(&game.join("native/build.gradle.kts"), "");
    write(&game.join("native/src/main/rust/Cargo.toml"), "");
    let (output, given) = forwarded(&root, &game, &["run"], 0);
    assert_eq!(given.unwrap(), ["run"]);
    let note = text(&output.stderr);
    assert!(
        note.starts_with("mutastic: tictactoe is a Gradle build, so the Java edition runs here. It also holds a Rust crate")
            && note.ends_with("\n  mutastic run native/src/main/rust\n"),
        "{note}"
    );
    let (_, given) = forwarded(
        &root,
        &game.join("native/src/main/rust"),
        &["gate", "open", "--hook=claude"],
        9,
    );
    assert_eq!(given, None);
    // Both build files in one directory: the person says which.
    write(&game.join("Cargo.toml"), "");
    let (output, given) = forwarded(&root, &game, &["run"], 0);
    assert_eq!((output.status.code(), given), (Some(2), None));
    assert!(text(&output.stderr).contains("say which with --edition=rust or --edition=java"));
    let (output, given) = forwarded(&root, &game, &["run", "--edition=java"], 0);
    assert_eq!(
        (output.status.code(), given.unwrap()),
        (Some(0), vec!["run".to_string()])
    );
}

/// A release directory as GitHub serves one, `download/v<version>/…`, with the Java edition's
/// archive in it, and a JDK that says it's the one given.
struct Releases {
    root: PathBuf,
    url: String,
    jdk: PathBuf,
}

impl Releases {
    fn new(root: &Path, jdk: u32) -> Releases {
        let dir = root.join("releases");
        let java = root.join(format!("jdk{jdk}"));
        executable(
            &java.join("bin/java"),
            &format!("#!/bin/sh\necho '    java.specification.version = {jdk}' >&2\n"),
        );
        Releases {
            url: format!("file://{}", dir.display()),
            root: dir,
            jdk: java,
        }
    }

    /// Publishes `version`, its SHA256SUMS saying `sum` for it, or its own checksum.
    fn publish(&self, version: &str, sum: Option<&str>) {
        let stage = self.root.join("stage");
        executable(
            &stage.join(format!("mutastic-{version}/bin/mutastic")),
            ENGINE,
        );
        let at = self.root.join(format!("download/v{version}"));
        std::fs::create_dir_all(&at).unwrap();
        let tar = at.join(format!("mutastic-{version}.tar"));
        let packed = Command::new("tar")
            .arg("-cf")
            .arg(&tar)
            .arg("-C")
            .arg(&stage)
            .arg(format!("mutastic-{version}"))
            .status()
            .unwrap();
        assert!(packed.success());
        std::fs::remove_dir_all(&stage).unwrap();
        let digest = sha2::Sha256::digest(std::fs::read(&tar).unwrap());
        let actual: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        write(
            &at.join("SHA256SUMS"),
            &format!("{}  mutastic-{version}.tar\n", sum.unwrap_or(&actual)),
        );
    }

    fn run(&self, dir: &Path, args: &[&str], version: &str) -> (Output, Option<Vec<String>>) {
        let record = dir.join("record");
        std::fs::remove_file(&record).ok();
        let output = mutastic(dir, args)
            .env("MUTASTIC_VERSION", version)
            .env("MUTASTIC_JAVA_RELEASES", &self.url)
            .env("JAVA_HOME", &self.jdk)
            .env("RECORD", &record)
            .output()
            .unwrap();
        (output, recorded(&record))
    }
}

#[test]
fn the_java_edition_is_fetched_once_checked_and_kept() {
    let root = scratch("fetch");
    let shop = root.join("shop");
    write(&shop.join("pom.xml"), "<project/>");
    let home = shop.join("home");
    let releases = Releases::new(&root, 25);
    // A Java patch of this native's own major.minor: from 1.0.0 that's the only line it runs.
    let java = format!(
        "{}.{}.9",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
    );
    releases.publish(&java, None);

    let version = mutastic(&shop, &["--version"]).output().unwrap();
    assert_eq!(
        text(&version.stdout).lines().nth(2),
        Some("Java edition: not fetched yet, which happens the first time mutastic runs in a Maven or Gradle project")
    );

    let (output, given) = releases.run(&shop, &["verify", "m855c8e3c"], &java);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(given.unwrap(), ["verify", "m855c8e3c"]);
    assert!(text(&output.stderr).contains(&format!("fetches its Java edition, {java}")));
    assert!(home.join(format!("mutastic-{java}/bin/mutastic")).is_file());
    let left: Vec<_> = std::fs::read_dir(&home)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left, [std::ffi::OsString::from(format!("mutastic-{java}"))]);

    // Kept: the next run fetches nothing, even with the release gone, and finds it unasked.
    std::fs::remove_dir_all(releases.root.join(format!("download/v{java}"))).unwrap();
    let (output, given) = releases.run(&shop, &["run"], "");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(given.unwrap(), ["run"]);
    assert_eq!(text(&output.stderr), "");
    let version = mutastic(&shop, &["--version"]).output().unwrap();
    assert_eq!(
        text(&version.stdout).lines().nth(2).unwrap(),
        format!(
            "Java edition {java}, at {}",
            home.join(format!("mutastic-{java}/bin/mutastic")).display()
        )
    );
}

#[test]
fn a_download_that_fails_its_checksum_or_a_jdk_too_old_keeps_nothing() {
    let root = scratch("refused");
    let shop = root.join("shop");
    write(&shop.join("pom.xml"), "<project/>");
    let home = shop.join("home");
    let releases = Releases::new(&root, 25);
    releases.publish("9.9.8", Some(&"0".repeat(64)));
    let (output, given) = releases.run(&shop, &["run"], "9.9.8");
    assert_eq!((output.status.code(), given), (Some(2), None));
    assert!(
        text(&output.stderr).contains("doesn't match its checksum"),
        "{}",
        text(&output.stderr)
    );
    assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);

    // The JDK is checked before anything is downloaded: there's no 9.9.7 to download.
    let old = Releases::new(&root.join("old"), 17);
    let (output, given) = old.run(&shop, &["run"], "9.9.7");
    assert_eq!((output.status.code(), given), (Some(2), None));
    assert!(
        text(&output.stderr).contains("the java found is JDK 17: set JAVA_HOME to a JDK 25"),
        "{}",
        text(&output.stderr)
    );
}

/// The installer, `scripts/install.sh`, on releases on disk as the release workflow lays them out:
/// one archive a target, and SHA256SUMS.
fn install(root: &Path, sum: Option<&str>) -> (Output, PathBuf) {
    let releases = root.join("releases");
    let at = releases.join("download/v9.9.9");
    std::fs::create_dir_all(&at).unwrap();
    let mut sums = String::new();
    for target in [
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
    ] {
        let name = format!("mutastic-9.9.9-{target}");
        let stage = root.join("stage");
        executable(
            &stage.join(&name).join("mutastic"),
            "#!/bin/sh\necho native 9.9.9\n",
        );
        executable(&stage.join(&name).join("cargo-mutastic"), "#!/bin/sh\n");
        let archive = at.join(format!("{name}.tar.gz"));
        let packed = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&stage)
            .arg(&name)
            .status()
            .unwrap();
        assert!(packed.success());
        std::fs::remove_dir_all(&stage).unwrap();
        let digest = sha2::Sha256::digest(std::fs::read(&archive).unwrap());
        let actual: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        sums.push_str(&format!("{}  {name}.tar.gz\n", sum.unwrap_or(&actual)));
    }
    write(&at.join("SHA256SUMS"), &sums);
    let bin = root.join("bin");
    let installer = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/install.sh");
    let output = Command::new("sh")
        .arg(installer)
        .env("HOME", root)
        .env("MUTASTIC_HOME", root.join("home"))
        .env("MUTASTIC_BIN_DIR", &bin)
        .env("MUTASTIC_NATIVE_VERSION", "9.9.9")
        .env(
            "MUTASTIC_NATIVE_RELEASES",
            format!("file://{}", releases.display()),
        )
        .output()
        .unwrap();
    (output, bin)
}

#[test]
fn the_installer_checks_the_native_command_and_links_it() {
    let root = scratch("install");
    let (output, bin) = install(&root, None);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("mutastic: installed 9.9.9:"));
    let ran = Command::new(bin.join("mutastic")).output().unwrap();
    assert_eq!(text(&ran.stdout), "native 9.9.9\n");
    assert!(bin.join("cargo-mutastic").is_file());

    let root = scratch("install-refused");
    let (output, bin) = install(&root, Some(&"0".repeat(64)));
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("doesn't match its checksum: not installed"));
    assert!(!bin.join("mutastic").exists());
}
