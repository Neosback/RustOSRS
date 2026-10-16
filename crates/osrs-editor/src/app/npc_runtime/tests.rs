use super::*;

#[test]
fn results_are_current_only_for_the_exact_identity() {
    let current = PlaytestIdentity {
        source_generation: 2,
        map_revision: 5,
        scenario_revision: 3,
        run_generation: 1,
    };
    assert!(identity_current(current, current));
    for stale in [
        PlaytestIdentity {
            source_generation: 1,
            ..current
        },
        PlaytestIdentity {
            map_revision: 4,
            ..current
        },
        PlaytestIdentity {
            scenario_revision: 2,
            ..current
        },
        PlaytestIdentity {
            run_generation: 0,
            ..current
        },
    ] {
        assert!(!identity_current(stale, current));
    }
}

#[test]
fn diagnostics_deduplicate_and_stay_bounded() {
    let mut runtime = NpcRuntime::default();
    for _ in 0..3 {
        runtime.report("same".into());
    }
    assert_eq!(runtime.diagnostics(), ["same".to_string()]);
    for n in 0..(MAX_DIAGNOSTICS * 2) {
        runtime.report(format!("message {n}"));
    }
    assert_eq!(runtime.diagnostics().len(), MAX_DIAGNOSTICS);
    assert_eq!(
        runtime.diagnostics().last().map(String::as_str),
        Some("message 127")
    );
}
