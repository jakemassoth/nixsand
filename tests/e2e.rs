/// End-to-end tests for nixsand.
///
/// All tests are `#[ignore]` by default. Run with:
///   cargo test -- --ignored
/// or set NIXSAND_E2E=1 and run `cargo test -- --ignored`.
///
/// Requirements: macOS aarch64, `container` (Apple), `tmux`, and `git` in PATH.
/// Heavy tests (project_branch_*) also build real container images.
use std::path::Path;
use std::process::Command;

use predicates::prelude::*;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const FLAKE_NIX: &str = include_str!("fixtures/sample-flake/flake.nix");
const FLAKE_LOCK: &str = include_str!("fixtures/sample-flake/flake.lock");

/// A sandboxed NIXSAND_HOME backed by a temp directory.
/// Uses a non-existent subdirectory so `nixsand init` sees a fresh home.
struct TestEnv {
    _tmp: TempDir,
    home: std::path::PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let tmp = TempDir::new().expect("create temp dir");
        let home = tmp.path().join("nixsand-home");
        TestEnv { _tmp: tmp, home }
    }

    fn home_path(&self) -> &Path {
        &self.home
    }

    fn cmd(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin("nixsand").unwrap();
        cmd.env("NIXSAND_HOME", &self.home);
        cmd
    }

    /// Run `nixsand init`, panicking on failure.
    fn init(&self) {
        self.cmd().arg("init").assert().success();
    }
}

/// A temporary local git repository seeded with the sample flake fixture.
struct SampleRepo {
    dir: TempDir,
}

impl SampleRepo {
    fn new() -> Self {
        let dir = TempDir::new().expect("create sample repo dir");
        let path = dir.path();

        std::fs::write(path.join("flake.nix"), FLAKE_NIX).unwrap();
        std::fs::write(path.join("flake.lock"), FLAKE_LOCK).unwrap();

        git(path, &["init", "-b", "main"]);
        git(path, &["config", "user.email", "test@nixsand.test"]);
        git(path, &["config", "user.name", "Test"]);
        git(path, &["add", "."]);
        git(path, &["commit", "-m", "initial"]);

        SampleRepo { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn url(&self) -> String {
        format!("file://{}", self.path().display())
    }

    #[allow(dead_code)]
    fn update_flake_lock(&self, content: &str) {
        std::fs::write(self.path().join("flake.lock"), content).unwrap();
        git(self.path(), &["add", "flake.lock"]);
        git(self.path(), &["commit", "-m", "update flake.lock"]);
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|_| panic!("git {:?} failed to spawn", args));
    assert!(status.success(), "git {:?} exited non-zero", args);
}

/// Unique project name: lowercase alphanumeric, safe for nixsand validation.
fn unique_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let pid = std::process::id();
    // Produces something like "t1a2b3c4d5e6" — all lowercase hex
    format!("t{:08x}{:04x}", pid, nanos & 0xffff)
}

/// Force-remove a container on drop (best-effort teardown for heavy tests).
struct ContainerCleanup(Vec<String>);

impl ContainerCleanup {
    fn new(names: impl IntoIterator<Item = String>) -> Self {
        ContainerCleanup(names.into_iter().collect())
    }
}

impl Drop for ContainerCleanup {
    fn drop(&mut self) {
        for name in &self.0 {
            let _ = Command::new("container")
                .args(["rm", "-f", name])
                .output();
        }
    }
}

/// Remove container images on drop (best-effort cleanup to avoid disk exhaustion).
struct ImageCleanup(Vec<String>);

impl ImageCleanup {
    fn new(names: impl IntoIterator<Item = String>) -> Self {
        ImageCleanup(names.into_iter().collect())
    }
}

impl Drop for ImageCleanup {
    fn drop(&mut self) {
        for name in &self.0 {
            let _ = Command::new("container")
                .args(["image", "rm", name])
                .output();
        }
    }
}

/// Kill tmux sessions on drop (best-effort teardown).
struct TmuxSessionCleanup(Vec<String>);

impl TmuxSessionCleanup {
    fn new(names: impl IntoIterator<Item = String>) -> Self {
        TmuxSessionCleanup(names.into_iter().collect())
    }
}

impl Drop for TmuxSessionCleanup {
    fn drop(&mut self) {
        for name in &self.0 {
            let _ = Command::new("tmux")
                .args(["kill-session", "-t", name])
                .output();
        }
    }
}

// ---------------------------------------------------------------------------
// init tests
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git; use `cargo test -- --ignored`"]
fn init_creates_expected_layout() {
    let env = TestEnv::new();

    env.cmd()
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("initialized"));

    assert!(
        env.home_path().join("projects").is_dir(),
        "projects/ dir should be created"
    );
    assert!(
        env.home_path().join("nixsand.db").is_file(),
        "nixsand.db should be created"
    );
}

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git"]
fn init_is_idempotent() {
    let env = TestEnv::new();
    env.cmd().arg("init").assert().success();
    env.cmd()
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

// ---------------------------------------------------------------------------
// project list tests
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git"]
fn project_list_empty() {
    let env = TestEnv::new();
    env.init();

    env.cmd()
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no projects"));
}

// ---------------------------------------------------------------------------
// project add tests
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git"]
fn project_add_registers_bare_clone_and_relative_paths() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success()
        .stdout(predicate::str::contains("added"));

    // Bare clone should exist
    let bare = env.home_path().join("projects").join(&name).join(".bare");
    assert!(bare.is_dir(), "bare repo should exist at {}", bare.display());

    // worktree.useRelativePaths = true
    let out = Command::new("git")
        .args(["-C"])
        .arg(&bare)
        .args(["config", "worktree.useRelativePaths"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "true",
        "worktree.useRelativePaths should be true"
    );

    // Project should appear in list
    env.cmd()
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&name));
}

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git"]
fn project_add_duplicate_name_rejected() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git"]
fn project_add_defaults_name_from_url() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();

    // URL ends in the temp dir basename; we just verify the command succeeds
    // and registers exactly one project.
    env.cmd()
        .args(["project", "add", &repo.url()])
        .assert()
        .success();

    let out = env.cmd().args(["project", "list"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<_> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 1, "exactly one project should be listed");
}

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git"]
fn project_add_invalid_name_rejected() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();

    // Names with uppercase, spaces, or slashes should fail validation
    for bad_name in &["Bad Name", "UPPER", "foo/bar", "-leading", "trailing-"] {
        env.cmd()
            .args(["project", "add", &repo.url(), bad_name])
            .assert()
            .failure();
    }
}

// ---------------------------------------------------------------------------
// project attach error tests (no container required)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git"]
fn project_attach_unknown_project_gives_clear_error() {
    let env = TestEnv::new();
    env.init();

    env.cmd()
        .args(["project", "attach", "nonexistent-proj", "main"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
#[ignore = "requires macOS aarch64 with container, tmux, git"]
fn project_attach_unknown_branch_gives_clear_error() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    env.cmd()
        .args(["project", "attach", &name, "no-such-branch"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ---------------------------------------------------------------------------
// project branch tests (heavy: builds images and containers)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn project_branch_creates_worktree_and_container() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    let container_name = format!("nixsand-{}-main", name);
    let _cleanup = ContainerCleanup::new([container_name.clone()]);
    let _image_cleanup = ImageCleanup::new([format!("nixsand-{}", name)]);

    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ready"));

    // Worktree at expected path
    let worktree = env
        .home_path()
        .join("projects")
        .join(&name)
        .join("worktrees")
        .join("main");
    assert!(worktree.is_dir(), "worktree should exist at {}", worktree.display());

    // Container should exist
    let inspect = Command::new("container")
        .args(["inspect", &container_name])
        .output()
        .expect("container inspect failed to spawn");
    assert!(inspect.status.success(), "container '{}' should exist", container_name);
}

#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn project_branch_is_idempotent() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    let container_name = format!("nixsand-{}-main", name);
    let _cleanup = ContainerCleanup::new([container_name]);
    let _image_cleanup = ImageCleanup::new([format!("nixsand-{}", name)]);

    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success();

    // Second invocation on the same branch should succeed
    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success();
}

#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn project_branch_flake_lock_unchanged_skips_image_rebuild() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    let c1 = format!("nixsand-{}-main", name);
    let c2 = format!("nixsand-{}-feature-two", name);
    let _cleanup = ContainerCleanup::new([c1, c2]);
    let _image_cleanup = ImageCleanup::new([format!("nixsand-{}", name)]);

    // First branch — builds images and records flake.lock hash
    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success();

    // Second branch on the same project, same flake.lock → image reuse
    let out = env.cmd()
        .args(["project", "branch", &name, "feature/two"])
        .output()
        .unwrap();
    assert!(out.status.success(), "second branch should succeed");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("up to date") || stderr.contains("skipping build"),
        "expected image-reuse message in stderr; got:\n{}",
        stderr
    );
}

#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn project_branch_flake_lock_changed_rebuilds_image() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    let c1 = format!("nixsand-{}-main", name);
    let c2 = format!("nixsand-{}-feature-three", name);
    let _cleanup = ContainerCleanup::new([c1, c2]);
    let _image_cleanup = ImageCleanup::new([format!("nixsand-{}", name)]);

    // First branch — records flake.lock hash in DB
    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success();

    // Inject a stale hash directly into the DB so the next branch sees a mismatch
    {
        use rusqlite::{params, Connection};
        let db_path = env.home_path().join("nixsand.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "UPDATE projects SET flake_lock_hash = 'stale-hash-that-does-not-match' WHERE name = ?1",
            params![name],
        )
        .unwrap();
    }

    // Second branch — hash mismatch → should rebuild per-project image
    let out = env.cmd()
        .args(["project", "branch", &name, "feature/three"])
        .output()
        .unwrap();
    assert!(out.status.success(), "branch after hash change should succeed");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("changed") || stderr.contains("rebuilding"),
        "expected rebuild message in stderr after hash mismatch; got:\n{}",
        stderr
    );
}

#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn project_branch_nonexistent_project_fails_cleanly() {
    let env = TestEnv::new();
    env.init();

    env.cmd()
        .args(["project", "branch", "no-such-project", "main"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));

    // No orphaned filesystem state should exist
    assert!(
        !env.home_path().join("projects").join("no-such-project").exists(),
        "no directory should be created for unknown project"
    );
}

// ---------------------------------------------------------------------------
// project attach happy-path test (heavy: builds images, creates tmux session)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn project_attach_creates_and_reattaches_tmux_session() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    let container_nm = format!("nixsand-{}-main", name);
    let session_nm = format!("nixsand_{}_main", name);
    let _container_cleanup = ContainerCleanup::new([container_nm.clone()]);
    let _image_cleanup = ImageCleanup::new([format!("nixsand-{}", name)]);
    let _session_cleanup = TmuxSessionCleanup::new([session_nm.clone()]);

    // Provision the branch so attach has a worktree + container to talk to.
    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success();

    // First attach: `tmux new-session -d` creates the session detached (no TTY
    // needed); the subsequent `tmux attach-session` fails because the test
    // process has no controlling TTY. We only care that the session was created.
    let first = env
        .cmd()
        .args(["project", "attach", &name, "main"])
        .output()
        .expect("nixsand attach failed to spawn");
    let first_stderr = String::from_utf8_lossy(&first.stderr);
    assert!(
        first_stderr.contains("creating new tmux session"),
        "first attach should log session creation; got stderr:\n{}",
        first_stderr
    );

    // Session must exist in tmux.
    let has = Command::new("tmux")
        .args(["has-session", "-t", &session_nm])
        .output()
        .expect("tmux has-session failed to spawn");
    assert!(
        has.status.success(),
        "tmux session '{}' should exist after first attach (stderr={})",
        session_nm,
        String::from_utf8_lossy(&has.stderr),
    );

    // Second attach: session already exists → should take the reattach branch.
    let second = env
        .cmd()
        .args(["project", "attach", &name, "main"])
        .output()
        .expect("nixsand attach failed to spawn");
    let second_stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        second_stderr.contains("reattaching to existing session"),
        "second attach should reuse the existing session; got stderr:\n{}",
        second_stderr
    );
}

// ---------------------------------------------------------------------------
// Sandbox functional test: `nix develop` must work inside the container
// ---------------------------------------------------------------------------

/// Regression test for the libgit2/`extensions.relativeWorktrees` issue.
///
/// When `worktree.useRelativePaths` is enabled on the bare repo, modern git
/// (2.48+) also writes `extensions.relativeWorktrees = true`. Nix uses libgit2,
/// which rejects unknown extensions and refuses to open the worktree — so
/// every `nix develop` inside the sandbox would die immediately with a
/// `libgit2 error code = 6` error. `project add` unsets the extension to keep
/// libgit2 happy; this test asserts that `nix develop` actually works inside
/// the running container, which is the user-facing guarantee that matters.
#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn nix_develop_works_inside_sandbox_container() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    let container_nm = format!("nixsand-{}-main", name);
    let _container_cleanup = ContainerCleanup::new([container_nm.clone()]);
    let _image_cleanup = ImageCleanup::new([format!("nixsand-{}", name)]);

    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success();

    // Run the same shape of command `nixsand project attach` runs (minus
    // claude itself, which would block waiting for input). If `nix develop`
    // can't open the worktree as a git repo, this fails fast with the
    // libgit2 extension-name error.
    let output = Command::new("container")
        .args([
            "exec",
            &container_nm,
            "sh",
            "-c",
            "cd /workspace/worktrees/main && nix develop -c echo NIX_DEVELOP_OK",
        ])
        .output()
        .expect("container exec failed to spawn");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success() && stdout.contains("NIX_DEVELOP_OK"),
        "`nix develop` inside the sandbox should succeed.\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout,
        stderr,
    );
}

/// Regression test for the "container runs as root" issue.
///
/// `claude --dangerously-skip-permissions` (the flag `project attach` launches
/// claude with) refuses to run under uid 0 with the message
/// "--dangerously-skip-permissions cannot be used with root/sudo privileges".
/// When that happens, the tmux pane's command exits in under a second and the
/// session dies (tmux's default `remain-on-exit off`), which is what the user
/// experiences as `[exited]` immediately after attaching.
///
/// This test invokes the real `nixsand project attach`, waits a few seconds,
/// and asserts the tmux session is still alive — meaning claude is actually
/// running inside the sandbox.
#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn claude_can_run_with_skip_permissions_inside_sandbox() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    let container_nm = format!("nixsand-{}-main", name);
    let session_nm = format!("nixsand_{}_main", name);
    let _container_cleanup = ContainerCleanup::new([container_nm.clone()]);
    let _image_cleanup = ImageCleanup::new([format!("nixsand-{}", name)]);
    let _session_cleanup = TmuxSessionCleanup::new([session_nm.clone()]);

    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success();

    // Run attach. The detached tmux session and the claude process inside it
    // are real even though the host `tmux attach-session` call fails due to no TTY.
    let _ = env
        .cmd()
        .args(["project", "attach", &name, "main"])
        .output()
        .expect("nixsand attach failed to spawn");

    // Give claude a few seconds to either start or exit fast with an error.
    std::thread::sleep(std::time::Duration::from_secs(4));

    // Best-effort pane capture for diagnostics in the failure message.
    // If the pane has already died, this returns nothing.
    let pane = Command::new("tmux")
        .args(["capture-pane", "-p", "-t", &session_nm])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    let has_session = Command::new("tmux")
        .args(["has-session", "-t", &session_nm])
        .output()
        .expect("tmux has-session failed to spawn")
        .status
        .success();

    assert!(
        !pane.contains("cannot be used with root"),
        "claude refused to start because the sandbox container runs as root.\npane:\n{}",
        pane,
    );
    assert!(
        has_session,
        "tmux session '{}' died within seconds of attach — claude exited immediately.\nlast captured pane:\n{}",
        session_nm, pane,
    );
}

/// End-to-end check of the user-facing attach flow: after `nixsand project
/// attach`, claude must have actually rendered its welcome screen in the
/// tmux pane. The earlier `claude_can_run_…` test only verifies that the
/// pane's command stays alive — but claude can stay alive without drawing
/// anything (no TTY, missing env, stuck initialization), which manifests
/// to the user as a blank terminal after attaching.
#[test]
#[ignore = "heavy: builds real container images; requires macOS aarch64 + container + tmux"]
fn attach_renders_claude_welcome_screen() {
    let env = TestEnv::new();
    env.init();

    let repo = SampleRepo::new();
    let name = unique_name();

    env.cmd()
        .args(["project", "add", &repo.url(), &name])
        .assert()
        .success();

    let container_nm = format!("nixsand-{}-main", name);
    let session_nm = format!("nixsand_{}_main", name);
    let _container_cleanup = ContainerCleanup::new([container_nm.clone()]);
    let _image_cleanup = ImageCleanup::new([format!("nixsand-{}", name)]);
    let _session_cleanup = TmuxSessionCleanup::new([session_nm.clone()]);

    env.cmd()
        .args(["project", "branch", &name, "main"])
        .assert()
        .success();

    // Run nixsand attach. The host-side `tmux attach-session` fails because
    // the test process has no controlling TTY, but the detached tmux session
    // and its claude process are real.
    let attach = env
        .cmd()
        .args(["project", "attach", &name, "main"])
        .output()
        .expect("nixsand attach failed to spawn");
    let attach_stderr = String::from_utf8_lossy(&attach.stderr).into_owned();

    // Give claude time to start, initialize, and draw its welcome screen.
    // `nix develop` is cached after the branch step, so this is mostly
    // claude's own startup time.
    std::thread::sleep(std::time::Duration::from_secs(15));

    // Gather diagnostics so a failure message tells us what state we're in.
    let tmux_ls = Command::new("tmux")
        .args(["ls"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let pane_list = Command::new("tmux")
        .args(["list-panes", "-t", &session_nm, "-F",
               "pid=#{pane_pid} cmd=#{pane_current_command} dead=#{pane_dead} size=#{pane_width}x#{pane_height}"])
        .output()
        .map(|o| {
            format!("{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr))
        })
        .unwrap_or_default();
    let container_procs = Command::new("container")
        .args(["exec", &container_nm, "sh", "-c",
               "ls /proc | grep '^[0-9]' | while read p; do c=$(tr '\\0' ' ' < /proc/$p/cmdline 2>/dev/null); [ -n \"$c\" ] && echo \"$p: $c\"; done"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    // Ownership of dirs sandbox needs to read/write — bind-mount filesystems
    // sometimes ignore chown.
    let ownership = Command::new("container")
        .args(["exec", &container_nm, "sh", "-c",
               "stat -c '%u:%g %n' /workspace /home/sandbox/.claude /nix/var/nix /nix/var/nix/db 2>&1"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    // Probe: run nix develop directly as uid 1000 and capture stderr.
    let probe = Command::new("container")
        .args(["exec", "--user", "1000", "-e", "HOME=/home/sandbox", &container_nm,
               "sh", "-lc",
               "cd /workspace/worktrees/main && nix develop -c true 2>&1; echo exit=$?"])
        .output()
        .map(|o| format!("{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)))
        .unwrap_or_default();
    let pane = Command::new("tmux")
        .args(["capture-pane", "-p", "-t", &session_nm])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    assert!(
        pane.contains("Claude"),
        "\n--- nixsand attach stderr ---\n{}\n\
         --- tmux ls ---\n{}\n\
         --- list-panes -t {} ---\n{}\n\
         --- container processes ---\n{}\n\
         --- container ownership ---\n{}\n\
         --- direct probe: nix develop as uid 1000 ---\n{}\n\
         --- captured pane (len={}) ---\n{}\n--- end ---",
        attach_stderr, tmux_ls, session_nm, pane_list, container_procs, ownership, probe, pane.len(), pane,
    );
}
