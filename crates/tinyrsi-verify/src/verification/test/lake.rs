//! Immutable Lake project staging and stock kernel-evidence requirements.
use super::*;

fn checkout_target(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(name)
}

fn fixture(version: &str, output: &str, exit: i32) -> (tempfile::TempDir, CommandSpec) {
    std::fs::create_dir_all(checkout_target("lake-fixtures")).unwrap();
    let dir = tempfile::tempdir_in(checkout_target("lake-fixtures")).unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    for (name, source) in [
        (
            "lean",
            format!(
                "#!/usr/bin/python3\nprint({:?})\n",
                format!("Lean (version {version}, fixture)")
            ),
        ),
        (
            "lake",
            format!("#!/usr/bin/python3\nimport sys\nprint({output:?})\nsys.exit({exit})\n"),
        ),
    ] {
        let path = root.join(name);
        std::fs::write(&path, source).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    let command = CommandSpec {
        program: root.join("lake").to_string_lossy().into_owned(),
        args: vec!["build".into()],
        workspace_root: root.join("workspaces"),
        timeout_ms: 5000,
        max_output_bytes: 4096,
    };
    (dir, command)
}

fn declaration(command: CommandSpec) -> VerifierDeclaration {
    VerifierDeclaration {
        node_id: "node".into(),
        owner_seat: "seat".into(),
        verifier: HostVerifier::Lake {
            command,
            theorem: "theorem rsi_goal : True :=".into(),
            toolchain: "leanprover/lean4:v4.34.1".into(),
        },
    }
}

#[test]
fn lake_version_build_and_output_failures_cannot_manufacture_proof_credit() {
    let report = "info: Rsi.lean:3:0: 'rsi_goal' does not depend on any axioms";
    for (version, output, exit) in [
        ("4.33.0", report, 0),
        ("4.34.1", report, 7),
        ("4.34.1", "Nothing to build.", 0),
    ] {
        let (_dir, command) = fixture(version, output, exit);
        let registry = VerifierRegistry::new(vec![declaration(command)]).unwrap();
        let binding = binding();
        let result = registry
            .verify(
                &binding,
                &ArtifactSubmission {
                    binding: binding.clone(),
                    content: b"by trivial".to_vec(),
                },
            )
            .unwrap();
        assert!(!result.passed);
        assert_eq!(result.quality, 0.0);
    }
    let (_dir, mut command) = fixture("4.34.1", report, 0);
    command.max_output_bytes = 1;
    let registry = VerifierRegistry::new(vec![declaration(command)]).unwrap();
    let binding = binding();
    assert!(
        !registry
            .verify(
                &binding,
                &ArtifactSubmission {
                    binding: binding.clone(),
                    content: b"by trivial".to_vec()
                }
            )
            .unwrap()
            .passed
    );
}

#[test]
fn lake_configuration_and_compiler_bytes_are_frozen_into_identity() {
    let (_dir, command) = fixture("4.34.1", "", 0);
    let stamp = crate::verification::lake::identity(&command).unwrap();
    let compiler = std::path::Path::new(&command.program).with_file_name("lean");
    std::fs::write(&compiler, b"changed compiler bytes").unwrap();
    assert_ne!(
        stamp,
        crate::verification::lake::identity(&command).unwrap()
    );
    let mut altered = command.clone();
    altered.args = vec!["build".into(), "other".into()];
    assert!(VerifierRegistry::new(vec![declaration(altered)]).is_err());
    let mut altered = declaration(command.clone());
    if let HostVerifier::Lake { theorem, .. } = &mut altered.verifier {
        *theorem = "theorem wrong : True := by trivial".into();
    }
    assert!(VerifierRegistry::new(vec![altered]).is_err());
    std::fs::remove_file(compiler).unwrap();
    assert!(VerifierRegistry::new(vec![declaration(command)]).is_err());
}

#[test]
fn lake_staging_pins_both_lake_and_compiler_bytes() {
    let (_dir, command) = fixture("4.34.1", "", 0);
    let identity = crate::verification::lake::identity(&command).unwrap();
    let staged = crate::verification::lake::stage(&command).unwrap();
    std::fs::write(&command.program, b"replaced lake").unwrap();
    std::fs::write(
        std::path::Path::new(&command.program).with_file_name("lean"),
        b"replaced lean",
    )
    .unwrap();
    assert_eq!(
        crate::verification::lake::identity_with_compiler(&command, Some(&staged.lean)).unwrap(),
        identity
    );
    assert_eq!(
        std::fs::read(&staged.lake).unwrap(),
        b"#!/usr/bin/python3\nimport sys\nprint(\"\")\nsys.exit(0)\n"
    );
    assert!(
        String::from_utf8(std::fs::read(&staged.lean).unwrap())
            .unwrap()
            .contains("Lean (version 4.34.1, fixture)")
    );
}

#[test]
#[cfg(unix)]
fn lake_identity_rejects_a_special_file_compiler() {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};

    let directory = tempfile::tempdir().unwrap();
    let lake = directory.path().join("lake");
    std::fs::write(&lake, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&lake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let lean = directory.path().join("lean");
    let _listener = UnixListener::bind(&lean).unwrap();
    let command = CommandSpec {
        program: lake.to_string_lossy().into_owned(),
        args: vec!["build".into()],
        workspace_root: directory.path().join("workspace"),
        timeout_ms: 1000,
        max_output_bytes: 1024,
    };
    assert!(crate::verification::lake::identity(&command).is_err());
}

#[test]
fn lake_reports_require_the_root_file_and_one_allowed_kernel_report() {
    for output in [
        b"info: Rsi.lean:3:0: 'rsi_goal' does not depend on any axioms\n".as_slice(),
        b"info: Rsi.lean:8:0: 'rsi_goal' depends on axioms: [propext, Classical.choice, Quot.sound]\n",
    ] { assert!(super::super::trusted_axioms(output, true)); }
    for output in [
        "Build completed successfully (3 jobs).",
        "info: Other.lean:3:0: 'rsi_goal' does not depend on any axioms",
        "info: Rsi.lean:0:0: 'rsi_goal' does not depend on any axioms",
        "info: Rsi.lean:x:0: 'rsi_goal' does not depend on any axioms",
        "info: Rsi.lean:3:x: 'rsi_goal' does not depend on any axioms",
        "info: Rsi.lean:3:0: 'rsi_goal' depends on axioms: [sorryAx]",
        "info: Rsi.lean:3:0: 'rsi_goal' depends on axioms: [rsi_goal._native.native_decide.ax_1_1]",
        "info: Rsi.lean:3:0: 'rsi_goal' depends on axioms: garbage",
        "info: Rsi.lean:3:0: 'rsi_goal' does not depend on any axioms\ninfo: Rsi.lean:4:0: 'rsi_goal' does not depend on any axioms",
    ] {
        assert!(!super::super::trusted_axioms(output.as_bytes(), true));
    }
}

#[test]
fn lake_rejects_alias_versions_shims_and_mutable_build_commands() {
    for toolchain in [
        "stable",
        "leanprover/lean4:stable",
        "leanprover/lean4:v4.34",
        "leanprover/lean4:v4.x.1",
        "leanprover/lean4:v4.34.1\n",
    ] {
        let command = CommandSpec {
            program: "/usr/bin/true".into(),
            args: vec!["build".into()],
            workspace_root: "target/rsi-workspaces".into(),
            timeout_ms: 5000,
            max_output_bytes: 4096,
        };
        assert!(
            VerifierRegistry::new(vec![VerifierDeclaration {
                node_id: "node".into(),
                owner_seat: "seat".into(),
                verifier: HostVerifier::Lake {
                    command,
                    theorem: "theorem rsi_goal : True :=".into(),
                    toolchain: toolchain.into()
                }
            }])
            .is_err()
        );
    }
}

#[test]
fn live_pinned_lake_builds_the_actual_kernel_proof_and_rejects_wrong_or_native_proofs() {
    let Ok(program) = std::env::var("TINYRSI_LAKE") else {
        return;
    };
    let command = CommandSpec {
        program,
        args: vec!["build".into()],
        workspace_root: checkout_target("lake-workspaces"),
        timeout_ms: 20000,
        max_output_bytes: 65536,
    };
    let registry = registry(HostVerifier::Lake {
        command,
        theorem: "import Std\ntheorem rsi_goal : (123 : Nat) % 10 = 3 :=".into(),
        toolchain: "leanprover/lean4:v4.34.1".into(),
    });
    let binding = binding();
    for (proof, expected) in [
        ("by decide", true),
        ("by rfl", true),
        ("by exact False.elim (by contradiction)", false),
        ("by sorry", false),
        ("by native_decide", false),
    ] {
        let evidence = registry
            .verify(
                &binding,
                &ArtifactSubmission {
                    binding: binding.clone(),
                    content: proof.as_bytes().to_vec(),
                },
            )
            .unwrap();
        assert_eq!(evidence.passed, expected);
        if expected {
            assert_eq!(evidence.exit_code, Some(0));
        }
    }
}

#[test]
fn lake_verifier_requires_a_frozen_project_and_stock_kernel_report() {
    std::fs::create_dir_all(checkout_target("lake-fixtures")).unwrap();
    let dir = tempfile::tempdir_in(checkout_target("lake-fixtures")).unwrap();
    let lake = dir.path().join("lake");
    let lean = dir.path().join("lean");
    std::fs::write(&lean, b"#!/bin/sh\necho 'Lean (version 4.34.1, fixture)'\n").unwrap();
    std::fs::write(&lake, b"#!/bin/sh\ntest \"$1\" = build || exit 2\ntest -f lakefile.toml || exit 3\ntest -f lean-toolchain || exit 4\ntest -f Rsi.lean || exit 5\ngrep -F 'defaultTargets' lakefile.toml >/dev/null || exit 6\necho \"info: Rsi.lean:3:0: 'rsi_goal' does not depend on any axioms\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&lake, &lean] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    let command = CommandSpec {
        program: lake.to_string_lossy().into_owned(),
        args: vec!["build".into()],
        workspace_root: dir.path().join("workspaces"),
        timeout_ms: 5000,
        max_output_bytes: 4096,
    };
    let registry = registry(HostVerifier::Lake {
        command,
        theorem: "import Std\ntheorem rsi_goal : 3 + 4 = 7 :=".into(),
        toolchain: "leanprover/lean4:v4.34.1".into(),
    });
    let binding = binding();
    let artifact = ArtifactSubmission {
        binding: binding.clone(),
        content: b"by decide".to_vec(),
    };
    let result = registry.verify(&binding, &artifact).unwrap();
    assert!(result.passed);
    assert_eq!(result.exit_code, Some(0));
}
