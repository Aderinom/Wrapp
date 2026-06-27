#!/usr/bin/env bash
# Executes multiple cargo commands in a workspace

push_script_dir() {
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    pushd "$SCRIPT_DIR" > /dev/null || exit 1
}
push_script_dir


# print help if -h or --help or help is passed at any position
for arg in "$@"; do
    if [[ "$arg" == "-h" || "$arg" == "--help" || "$arg" == "help" ]]; then
			echo -e "\e[36mBatch Commands:\e[0m
			- \e[33mcheck\e[0m (runs fmt, lint, doc, shear, tests in check mode)
			- \e[33mfix\e[0m   (runs fmt, lint, shear in fix mode)
			"
			echo -e "\e[36mAliases:\e[0m
			- \e[33mfmt\e[0m  (for +nightly fmt)
			- \e[33mdoc\e[0m  (for doc with CI settings)
			- \e[33mlint\e[0m (for clippy with CI settings)
			- \e[33mtest\e[0m (for a workspace-wide test run)
			"
			exit 0
		fi
done

# if no arguments are passed, fallback to check
if [ $# -eq 0 ]; then
    set -- "check"
fi

########################
# Batch Commands:
declare -a RESULTS
FAILED=0

# resolve aliases (fmt/doc/lint/test) into the full cargo arg list in the REPLY array;
cargo_alias() {
    case "$1" in
        # fmt should always use +nightly
        fmt)  REPLY=(+nightly fmt "${@:2}") ;;
        # doc gets the full documentation argument set
        doc)  REPLY=(doc --workspace --all-features --no-deps --document-private-items "${@:2}") ;;
        # lint is clippy with the workspace-wide settings
        lint) REPLY=(clippy --all-targets "${@:2}" -- -D warnings) ;;
        # test is a workspace-wide test run
        test) REPLY=(test --all "${@:2}") ;;
        *)    REPLY=("$@") ;;
    esac
}

# run_cargo resolves an alias, echoes the "▶ cargo …" line, then runs it live.
run_cargo() {
    cargo_alias "$@"
    printf '\e[36m▶ cargo %s\e[0m\n' "${REPLY[*]}"
    cargo "${REPLY[@]}"
}

# run_step runs one step live (echo + output) and records pass/fail for the
# summary. Used by the sequential `fix` path.
run_step() {
    local name="$1"; shift
    local start=$SECONDS
    if run_cargo "$@"; then
        printf '\e[32m✓ %s\e[0m (%ds)\n\n' "$name" "$((SECONDS - start))"
        RESULTS+=("  \e[32m✓\e[0m $name")
    else
        printf '\e[31m✗ %s\e[0m (%ds)\n\n' "$name" "$((SECONDS - start))"
        RESULTS+=("  \e[31m✗\e[0m $name")
        FAILED=1
    fi
}

# launch_step echoes the resolved "▶ cargo …" line live, then starts the step in
# the background, buffering the command's output to a per-step log and recording
# its own wall-clock duration.
launch_step() {
    local name="$1"; shift
    local idx=${#PIDS[@]}
    cargo_alias "$@"
    printf '\e[36m▶ cargo %s\e[0m\n' "${REPLY[*]}"
    NAMES[idx]="$name"
    LOGS[idx]="$STEP_TMPDIR/step_$idx.log"
    DURS[idx]="$STEP_TMPDIR/step_$idx.dur"
    local cmd=("${REPLY[@]}")
    (
        s=$(date +%s)
				
        RUSTDOCFLAGS="-D warnings" cargo "${cmd[@]}" > "${LOGS[idx]}" 2>&1
        rc=$?
        printf '%s' "$(( $(date +%s) - s ))" > "${DURS[idx]}"
        exit "$rc"
    ) &
    PIDS[idx]=$!
}

# wait_steps waits for every launched step (in launch order) and reports each
# result: quiet on success, full buffered log on failure.
wait_steps() {
    local i status dur
    for i in "${!PIDS[@]}"; do
        if wait "${PIDS[i]}"; then status=0; else status=1; fi
        dur="$(cat "${DURS[i]}" 2>/dev/null)"
        if [ "$status" -eq 0 ]; then
            printf '\e[32m✓ %s\e[0m (%ss)\n' "${NAMES[i]}" "$dur"
            RESULTS+=("  \e[32m✓\e[0m ${NAMES[i]}")
        else
            printf '\e[31m✗ %s\e[0m (%ss)\n' "${NAMES[i]}" "$dur"
            cat "${LOGS[i]}"
            RESULTS+=("  \e[31m✗\e[0m ${NAMES[i]}")
            FAILED=1
        fi
    done
}

# All Checks
if [ "$1" = "check" ]; then
    # Per-run scratch space for buffered step output; only the parallel `check`
    # path needs it, so it's created here rather than at the top of the script.
    STEP_TMPDIR="$(mktemp -d)"
    trap 'rm -rf "$STEP_TMPDIR"' EXIT
    declare -a PIDS=() NAMES=() LOGS=() DURS=()

    launch_step "format" fmt --all -- --check
    launch_step "doc"    doc --quiet --frozen
    launch_step "shear"  shear
    launch_step "lint"   lint --quiet
    launch_step "test"   test --quiet --no-fail-fast
    launch_step "deny"   deny check
    echo -e "\e[36m▶ running ${#PIDS[@]} checks in parallel…\e[0m"
    wait_steps

    echo "──────── Summary ────────"
    printf '%b\n' "${RESULTS[@]}"
    if [ "$FAILED" -eq 0 ]; then
        echo -e "\e[32mAll checks passed ✅\e[0m"
    else
        echo -e "\e[31mSome checks failed ❌\e[0m"
    fi

    exit "$FAILED"
fi


# All Fixes 
if [ "$1" = "fix" ]; then
    # Sequentially run all fixable steps, recording pass/fail for the summary.
    run_step "lint --fix"   lint --fix --allow-dirty
    run_step "format"       fmt --all
    run_step "shear --fix"  shear --fix

    echo "──────── Changed files ────────"
    git --no-pager diff --stat

    echo "──────── Summary ────────"
    printf '%b\n' "${RESULTS[@]}"
    [ "$FAILED" -eq 0 ] \
        && echo -e "\e[32mAll fixes applied ✅\e[0m" \
        || echo -e "\e[31mSome fixes failed ❌\e[0m"

    exit "$FAILED"
fi

##################
# Direct invocation: expand any alias (see run_cargo) and run it.
run_cargo "$@"
if [ $? -ne 0 ]; then
		echo -e "\e[31mCommand\e[0m $1 \e[31mfailed\e[0m"
		exit 1
fi
