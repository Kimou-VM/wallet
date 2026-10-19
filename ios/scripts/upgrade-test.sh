#!/usr/bin/env bash
set -euo pipefail

COMMIT="${1:?Usage: just test-upgrade <commit-or-tag>}"
REPO_ROOT="$(git rev-parse --show-toplevel)"
IOS_DIR="$REPO_ROOT/ios"
WORKTREE_DIR="/tmp/gem-upgrade-test-$$"
OLD_IOS_DIR="$WORKTREE_DIR/ios"
export SIMULATOR_NAME="${SIMULATOR_NAME:-iPhone 17}"
SIMULATOR_DEST="platform=iOS Simulator,name=$SIMULATOR_NAME"

cleanup() {
    echo "==> Cleaning up worktree"
    cd "$REPO_ROOT"
    git worktree remove --force "$WORKTREE_DIR" 2>/dev/null || true
}
trap cleanup EXIT

run_ui_test() {
    local test_class="$1"
    set -o pipefail && xcodebuild -project Gem.xcodeproj \
        -scheme GemUITests \
        -testPlan ui_tests \
        -derivedDataPath build/DerivedData \
        -clonedSourcePackagesDirPath build/SourcePackages \
        -destination "$SIMULATOR_DEST" \
        -only-testing "GemUITests/$test_class" \
        test-without-building | xcbeautify --quieter --is-ci
}

echo "==> Phase 1: Build old version ($COMMIT)"

git worktree add --detach "$WORKTREE_DIR" "$COMMIT"
cd "$WORKTREE_DIR"
git submodule update --init

cd "$OLD_IOS_DIR"
just generate-stone
just build-for-testing-integration

echo "==> Resetting simulator"
just reset-simulator

echo "==> Running ImportWalletReceiveBitcoinUITests (old version)"
run_ui_test ImportWalletReceiveBitcoinUITests

echo "==> Phase 2: Build current version"
cd "$IOS_DIR"
just generate-stone
just build-for-testing-integration

echo "==> Running UpgradeVerificationTests (current version)"
TEST_RUNNER_GEM_UPGRADE_TEST=1 run_ui_test UpgradeVerificationTests

echo "==> Upgrade test passed!"
