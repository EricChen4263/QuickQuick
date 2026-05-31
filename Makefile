# QuickQuick 构建流程入口 / build workflow entrypoint
# 两类用途：① 本地快速验证（doctor/dev/check/test/verify）② 出本地发布包（build）
# 发布给别人走 GitHub Actions（打 v* tag 触发），见 .github/workflows/release.yml。

SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

# Universal mac 构建需要两个 darwin target
MAC_UNIVERSAL_TARGET := universal-apple-darwin

.PHONY: help doctor dev fmt check test test-js test-rs verify build bump clean

help:
	@echo "QuickQuick make 目标："
	@echo "  make doctor              环境体检：工具链 + rust target 齐不齐"
	@echo "  make dev                 热重载跑应用（tauri dev）"
	@echo "  make fmt                 自动格式化 Rust 代码（cargo fmt）"
	@echo "  make check               极速体检：tsc 类型 + cargo check（不跑不打包）"
	@echo "  make test                全测并行：vitest + cargo test"
	@echo "  make verify              提交门禁：类型 + fmt + clippy + 全测（CI 复用）"
	@echo "  make build               本地出 Universal mac .dmg/.app"
	@echo "  make bump VERSION=x.y.z  同步三处版本号（package.json/tauri.conf.json/Cargo.toml）"
	@echo "  make clean               清构建产物（dist + cargo target）"

doctor:
	@echo "== 环境体检 =="
	@ok=1; \
	for c in node pnpm cargo rustc rustup; do \
	  if command -v $$c >/dev/null 2>&1; then printf "  ✓ %-8s %s\n" "$$c" "$$($$c --version 2>&1 | head -1)"; \
	  else printf "  ✗ %-8s 未安装\n" "$$c"; ok=0; fi; \
	done; \
	if pnpm tauri --version >/dev/null 2>&1; then printf "  ✓ %-8s %s\n" "tauri" "$$(pnpm tauri --version 2>&1 | tail -1)"; \
	else printf "  ✗ %-8s 未安装（pnpm install 装 @tauri-apps/cli）\n" "tauri"; ok=0; fi; \
	echo "== Universal mac 所需 rust target =="; \
	for t in aarch64-apple-darwin x86_64-apple-darwin; do \
	  if rustup target list --installed 2>/dev/null | grep -qx "$$t"; then printf "  ✓ %s\n" "$$t"; \
	  else printf "  ✗ %s  → 补：rustup target add %s\n" "$$t" "$$t"; ok=0; fi; \
	done; \
	[ "$$ok" = 1 ] && echo "环境就绪。" || { echo "有缺项，按上面提示补齐后重跑 make doctor。"; exit 1; }

dev:
	pnpm tauri dev

fmt:
	cd src-tauri && cargo fmt --all
	@echo "Rust 代码已格式化（make verify 的 fmt 门禁随之转绿）。"

check:
	@echo "== 类型检查 (tsc) =="
	pnpm exec tsc -b
	@echo "== Rust 编译检查 (cargo check) =="
	cd src-tauri && cargo check

test-js:
	pnpm test

test-rs:
	cd src-tauri && cargo test

# 并行跑前端与 Rust 测试，任一失败则整体失败
test:
	@echo "== 并行全测：vitest + cargo test =="
	@set -uo pipefail; \
	pnpm test & js=$$!; \
	( cd src-tauri && cargo test ) & rs=$$!; \
	fail=0; \
	wait $$js || fail=1; \
	wait $$rs || fail=1; \
	[ "$$fail" = 0 ] && echo "全测通过。" || { echo "有测试失败（见上）。"; exit 1; }

# 提交前门禁：与 CI release 工作流复用同一条标准
verify:
	@echo "== [1/5] 前端类型 (tsc) =="
	pnpm exec tsc -b
	@echo "== [2/5] Rust 格式 (cargo fmt --check) =="
	cd src-tauri && cargo fmt --all --check
	@echo "== [3/5] Rust 静态检查 (cargo clippy) =="
	cd src-tauri && cargo clippy --all-targets -- -D warnings
	@echo "== [4/5] 前端测试 (vitest) =="
	pnpm test
	@echo "== [5/5] Rust 测试 (cargo test) =="
	cd src-tauri && cargo test
	@echo "verify 全过，可提交/发布。"

build:
	@echo "== 出 Universal mac 安装包（首次需 rustup target add x86_64-apple-darwin）=="
	pnpm tauri build --target $(MAC_UNIVERSAL_TARGET)
	@echo "产物在 src-tauri/target/$(MAC_UNIVERSAL_TARGET)/release/bundle/"

# 同步版本号到三处，避免 tag 与配置不一致（release 工作流会校验，不一致直接 fail）
bump:
	@test -n "$(VERSION)" || { echo "用法: make bump VERSION=x.y.z"; exit 1; }
	@perl -i -pe 'if(!$$d && s/("version":\s*)"[^"]*"/$$1"$(VERSION)"/){$$d=1}' package.json
	@perl -i -pe 'if(!$$d && s/("version":\s*)"[^"]*"/$$1"$(VERSION)"/){$$d=1}' src-tauri/tauri.conf.json
	@perl -i -pe 'if(!$$d && s/^(version = )"[^"]*"/$$1"$(VERSION)"/){$$d=1}' src-tauri/Cargo.toml
	@echo "已同步版本号 → $(VERSION)（package.json / tauri.conf.json / Cargo.toml）"
	@echo "下一步：git commit -am 'chore: bump v$(VERSION)' && git tag v$(VERSION) && git push --tags"

clean:
	rm -rf dist
	cd src-tauri && cargo clean
	@echo "已清 dist 与 cargo target（node_modules 保留，删可手动 rm -rf node_modules）。"
