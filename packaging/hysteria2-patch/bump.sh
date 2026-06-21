#!/bin/bash
# Переключает пин версии нашего форка hysteria2 (см. build.sh) на новый
# тег апстрима apernet/hysteria. Сам НЕ собирает и НЕ публикует — только
# правит версию в build.sh и заранее проверяет, что direct-domains.patch
# ещё применяется к новому тегу, чтобы не проставить версию, для которой
# build.sh потом молча упадёт или соберётся без directDomains.
#
# Использование:
#   ./bump.sh app/v2.9.3        # ревизия патча 1 (по умолчанию)
#   ./bump.sh app/v2.9.3 2      # явная ревизия (если для того же апстрима
#                                # переделываем сам патч повторно)
set -e

if [[ -z "$1" ]]; then
    echo "Использование: $0 <upstream-tag> [patch-revision]"
    echo "Пример:        $0 app/v2.9.3"
    exit 1
fi

NEW_TAG="$1"
NEW_REVISION="${2:-1}"

if [[ "$NEW_TAG" != app/* ]]; then
    echo "✗ Тег апстрима должен быть вида app/vX.Y.Z (как теги apernet/hysteria), получено: $NEW_TAG"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_SH="$SCRIPT_DIR/build.sh"

NEW_VERSION="${NEW_TAG#app/}-vroxory${NEW_REVISION}"
NEW_RELEASE_TAG="hysteria2-fork-${NEW_TAG#app/}-${NEW_REVISION}"

echo "═══════════════════════════════════"
echo "  Обновление пина hysteria2-fork"
echo "  ${NEW_TAG} (ревизия патча ${NEW_REVISION})"
echo "  -> версия: ${NEW_VERSION}, тег релиза: ${NEW_RELEASE_TAG}"
echo "═══════════════════════════════════"

# ── 1. Проверяем, что патч ещё применяется к новому тегу апстрима ──
CHECK_DIR="$(mktemp -d)"
ERR_LOG="$(mktemp)"
trap 'rm -rf "$CHECK_DIR" "$ERR_LOG"' EXIT

echo ""
echo "▶ Проверяю применимость патча к ${NEW_TAG}..."
git clone --quiet --depth 1 https://github.com/apernet/hysteria.git "$CHECK_DIR/hysteria"
cd "$CHECK_DIR/hysteria"
if ! git fetch --quiet --depth 1 origin tag "$NEW_TAG" 2>"$ERR_LOG"; then
    echo "✗ Тег ${NEW_TAG} не найден в apernet/hysteria:"
    cat "$ERR_LOG"
    exit 1
fi
git checkout --quiet "$NEW_TAG"

if ! git apply --check --include="app/cmd/client.go" "$SCRIPT_DIR/direct-domains.patch" 2>"$ERR_LOG" \
   || ! git apply --check --include="app/internal/tun/server.go" "$SCRIPT_DIR/direct-domains.patch" 2>>"$ERR_LOG"; then
    echo "✗ direct-domains.patch не применяется к ${NEW_TAG}:"
    cat "$ERR_LOG"
    echo ""
    echo "  Апстрим, видимо, поменял app/cmd/client.go или app/internal/tun/server.go —"
    echo "  нужно вручную обновить direct-domains.patch перед bump."
    exit 1
fi
cd "$SCRIPT_DIR"
echo "✓ Патч применяется без конфликтов"

# ── 2. Правим build.sh ──
sed -i "s|^UPSTREAM_TAG=.*|UPSTREAM_TAG=\"${NEW_TAG}\"|" "$BUILD_SH"
sed -i "s|^PATCH_REVISION=.*|PATCH_REVISION=\"${NEW_REVISION}\"|" "$BUILD_SH"

echo ""
echo "✓ Обновлено:"
echo "  packaging/hysteria2-patch/build.sh -> UPSTREAM_TAG=${NEW_TAG}, PATCH_REVISION=${NEW_REVISION}"
echo ""
echo "Дальше:"
echo "  1. ./build.sh             — собрать и проверить бинарник локально"
echo "  2. ./build.sh --publish   — опубликовать релиз ${NEW_RELEASE_TAG} в GitHub"
