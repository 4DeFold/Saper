#!/usr/bin/env bash
# Все проверки Rust одним запуском. Полный вывод пишется в ci-report.txt,
# чтобы его можно было прочитать из GitHub Actions (логи/артефакты могут быть
# недоступны в песочнице разработчика, поэтому отчёт ещё публикуется
# комментарием к коммиту).
set -uo pipefail

REPORT="${CI_REPORT:-ci-report.txt}"
: > "$REPORT"

log() {
  echo "$*" | tee -a "$REPORT"
}

log "=== toolchain ==="
log "rustc:   $(rustc -V)"
log "cargo:   $(cargo -V)"
log "rustfmt: $(rustfmt -V)"
log "clippy:  $(cargo clippy -V)"
log "uname:   $(uname -a)"

failed=0
run() {
  log ""
  log "===== $* ====="
  "$@" >> "$REPORT" 2>&1
  local rc=$?
  log "exit=$rc"
  if [[ $rc -ne 0 ]]; then
    failed=1
  fi
}

run cargo fmt --all -- --check
run cargo check --workspace --all-targets
run cargo test --workspace
run cargo clippy --workspace --all-targets -- -D warnings
run cargo build --release --workspace

# Если rustfmt нашёл отличия, приложить готовый патч к отчёту: его можно
# применить локально через git apply без ручного переформатирования.
if ! cargo fmt --all -- --check >/dev/null 2>&1; then
  log ""
  log "===== RUSTFMT_PATCH_BEGIN ====="
  cargo fmt --all
  git diff --binary -- '*.rs' >> "$REPORT"
  log "===== RUSTFMT_PATCH_END ====="
fi

log ""
log "===== SUMMARY ====="
log "failed=$failed"
exit "$failed"
