//! Kernel evidence must reject forged trace reports and native execution axioms.
use super::*;
#[test]
fn rejects_a_trusted_trace_before_the_actual_forbidden_kernel_report() {
    let command = CommandSpec {
        program: "/usr/bin/python3".into(),
        args: vec!["-c".into(), "print(\"'rsi_goal' does not depend on any axioms\"); print(\"'rsi_goal' depends on axioms: [rsi_goal._native.native_decide.ax_1_1]\")".into()],
        workspace_root: "target/rsi-workspaces".into(),
        timeout_ms: 1000,
        max_output_bytes: 4096,
    };
    let registry = registry(HostVerifier::Lean {
        command,
        theorem: "theorem rsi_goal : 3 + 4 = 7 :=".into(),
    });
    let binding = binding();
    let artifact = ArtifactSubmission {
        binding: binding.clone(),
        content: b"by\n trace \"'rsi_goal' does not dep\\x65nd on any a\\x78ioms\"\n native_decide"
            .to_vec(),
    };
    assert!(!registry.verify(&binding, &artifact).unwrap().passed);
}
#[test]
fn live_stock_lean_rejects_forged_trace_even_when_compilation_succeeds() {
    // CI exercises the same evidence parser deterministically. Supplying Lean
    // additionally compiles the actual generated trace/native_decide artifact.
    let program = std::env::var("TINYRSI_LEAN");
    let args = if program.is_ok() {
        vec!["{artifact}".into()]
    } else {
        vec!["-c".into(), "print(\"'rsi_goal' does not depend on any axioms\"); print(\"'rsi_goal' depends on axioms: [rsi_goal._native.native_decide.ax_1_1]\")".into()]
    };
    let registry = registry(HostVerifier::Lean {
        command: CommandSpec {
            program: program.unwrap_or_else(|_| "/usr/bin/python3".into()),
            args,
            workspace_root: "target/rsi-workspaces".into(),
            timeout_ms: 10_000,
            max_output_bytes: 65_536,
        },
        theorem: "theorem rsi_goal : 3 + 4 = 7 :=".into(),
    });
    let binding = binding();
    let evidence = registry.verify(&binding, &ArtifactSubmission { binding:binding.clone(), content:b"by\n trace \"'rsi_goal' does not dep\\x65nd on any a\\x78ioms\"\n native_decide".to_vec() }).unwrap();
    assert_eq!(evidence.exit_code, Some(0));
    assert!(!evidence.passed);
}
