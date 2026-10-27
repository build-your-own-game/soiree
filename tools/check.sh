#!/usr/bin/env bash
# Vérifie qu'un projet respecte ce que le manifeste permet de vérifier par machine.
#
#   tools/check.sh                    tous les projets du dépôt
#   tools/check.sh games/quiplash     un seul projet
#   tools/check.sh --fast ...         sans fmt, clippy ni tests
#
# Le socle (toolchain, lints, clippy.toml) est lu dans MANIFESTE.md lui-même :
# durcir le manifeste suffit pour que tous les projets soient revérifiés.
# Ce qui demande du jugement (états impossibles, sensation, validité d'une
# raison) reste hors de portée ; le script exige seulement qu'il soit écrit,
# dans la section « Écarts au manifeste » de LEARNINGS.md.

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="$ROOT/MANIFESTE.md"
FAST=0
ERRORS=0
WARNINGS=0

# Dépendances qui n'ont rien à faire dans une simulation qui tourne sans écran.
SCREEN_CRATES='^(web-sys|js-sys|wasm-bindgen|wasm-bindgen-futures|wgpu|winit|web-time|gloo[a-z-]*) v'

err() { printf '  ✗ %s\n' "$1"; ERRORS=$((ERRORS + 1)); }
warn() { printf '  ! %s\n' "$1"; WARNINGS=$((WARNINGS + 1)); }
ok() { printf '  ✓ %s\n' "$1"; }

# Le bloc ```toml du manifeste qui contient $1.
manifest_block() {
    awk -v needle="$1" '
        /^```toml/ { inblock = 1; block = ""; next }
        /^```/ && inblock { inblock = 0; if (index(block, needle)) { printf "%s", block; exit } next }
        inblock { block = block $0 "\n" }
    ' "$MANIFEST"
}

# Une ligne TOML sans espaces superflus, pour comparer.
normalize() { sed -e 's/#.*//' -e 's/[[:space:]]//g' -e '/^$/d'; }

check_toolchain() {
    local dir="$1" expected actual
    if [ ! -f "$dir/rust-toolchain.toml" ]; then
        err "rust-toolchain.toml absent"
        return
    fi
    expected=$(manifest_block '[toolchain]' | normalize | grep '^channel=')
    actual=$(normalize < "$dir/rust-toolchain.toml" | grep '^channel=')
    [ "$expected" = "$actual" ] || err "rust-toolchain.toml : $actual au lieu de $expected"
    grep -q 'wasm32-unknown-unknown' "$dir/rust-toolchain.toml" || err "rust-toolchain.toml : cible wasm32-unknown-unknown absente"
    grep -q 'clippy' "$dir/rust-toolchain.toml" || err "rust-toolchain.toml : composant clippy absent"
    grep -q 'rustfmt' "$dir/rust-toolchain.toml" || err "rust-toolchain.toml : composant rustfmt absent"
}

check_cargo() {
    local dir="$1" project line missing=0 crate
    project=$(normalize < "$dir/Cargo.toml")
    # Chaque « deny » du manifeste doit y être ; un « allow » peut disparaître (durcir est libre).
    while IFS= read -r line; do
        case "$line" in
            \[*) continue ;;
            *'"allow"'*) continue ;;
            name=* | version=*) continue ;;
        esac
        if ! printf '%s\n' "$project" | grep -qxF "$line"; then
            err "Cargo.toml : il manque \`$line\`"
            missing=1
        fi
    done < <(manifest_block '[lints.rust]' | normalize)
    [ "$missing" = 0 ] && ok "lints, édition et profils du manifeste"

    for crate in "$dir"/crates/*/Cargo.toml "$dir"/xtask/Cargo.toml; do
        [ -f "$crate" ] || continue
        if ! normalize < "$crate" | awk '/^\[lints\]$/ { getline n; if (n == "workspace=true") found = 1 } /^lints\.workspace=true$/ { found = 1 } END { exit !found }'; then
            err "${crate#"$ROOT"/} n'hérite pas des lints du workspace ([lints] workspace = true)"
        fi
    done
}

check_clippy_toml() {
    local dir="$1" line missing=0
    if [ ! -f "$dir/clippy.toml" ]; then
        err "clippy.toml absent"
        return
    fi
    while IFS= read -r line; do
        if ! grep -qF "$line" "$dir/clippy.toml"; then
            err "clippy.toml : il manque $line"
            missing=1
        fi
    done < <(manifest_block 'disallowed-types' | grep -oE '(path = "[^"]+"|^[a-z-]+-in-tests = true)')
    [ "$missing" = 0 ] && ok "types et méthodes interdits du manifeste"
}

# Principe 4 : la simulation compile sans GPU ni navigateur.
check_headless_core() {
    local dir="$1" name found
    if [ ! -f "$dir/crates/core/Cargo.toml" ]; then
        warn "pas de crates/core : impossible de vérifier que la simulation tourne sans écran"
        return
    fi
    name=$(awk -F'"' '/^name *=/ { print $2; exit }' "$dir/crates/core/Cargo.toml")
    found=$(cd "$dir" && cargo tree -q -p "$name" -e normal --target all --prefix none 2>/dev/null | grep -E "$SCREEN_CRATES" | sort -u | tr '\n' ' ')
    if [ -n "$found" ]; then
        err "$name dépend de $found: la simulation doit compiler sans navigateur ni GPU"
    else
        ok "$name ne dépend ni du navigateur ni du GPU"
    fi
}

# Le JavaScript se limite à la colle générée et au strict nécessaire.
check_javascript() {
    local dir="$1" file
    [ -d "$dir/web" ] || return
    while IFS= read -r file; do
        err "JavaScript écrit à la main dans ${file#"$ROOT"/} : passe par wasm-bindgen"
    done < <(find "$dir/web" -type f \( -name '*.js' -o -name '*.mjs' -o -name '*.ts' \) -not -path '*/pkg/*')
}

# Les chemins cités dans AGENTS.md et README.md existent.
check_doc_paths() {
    local dir="$1" doc path
    for doc in "$dir/AGENTS.md" "$dir/README.md"; do
        [ -f "$doc" ] || continue
        while IFS= read -r path; do
            [ -e "$dir/$path" ] || err "${doc#"$ROOT"/} cite \`$path\`, qui n'existe pas"
        done < <(grep -oE '`[A-Za-z0-9_.][A-Za-z0-9_./-]*/[A-Za-z0-9_./-]*`' "$doc" | tr -d '`' | grep -E '[A-Za-z]' | sort -u)
    done
}

# Chaque principe d'ingénierie est respecté, ou son écart est écrit avec sa raison.
check_gaps() {
    local dir="$1" learnings="$1/LEARNINGS.md" number text
    if [ ! -f "$learnings" ]; then
        err "LEARNINGS.md absent"
        return
    fi
    if ! grep -q '^## Écarts au manifeste' "$learnings"; then
        err "LEARNINGS.md : section « ## Écarts au manifeste » absente (un point par principe d'ingénierie, 1 à 7)"
        return
    fi
    while IFS="$(printf '\t')" read -r number text; do
        case "$text" in
            *'**Respecté'*)
                if [ "$number" = 5 ] && [ -d "$dir/web" ] && ! grep -rqi 'Cross-Origin-Embedder-Policy' "$dir/crates"; then
                    err "principe 5 marqué respecté, mais aucun crate n'envoie l'en-tête COOP/COEP"
                fi
                ;;
            *'**Écart'*)
                case "$text" in
                    *Raison*) ;;
                    *) err "principe $number : écart sans « Raison : »" ;;
                esac
                ;;
            *'**À faire'*) warn "principe $number à faire : $(printf '%s' "$text" | sed -E 's/^[0-9]+\. \*\*À faire\.?\*\*\.? *//' | cut -c1-110)" ;;
            '') err "principe $number absent de « Écarts au manifeste »" ;;
            *) err "principe $number : commence par **Respecté.**, **Écart.** ou **À faire.**" ;;
        esac
    done < <(awk '
        /^## Écarts au manifeste/ { s = 1; next }
        s && /^## / { s = 0 }
        !s { next }
        /^[0-9]+\. / { cur = $0; sub(/\..*/, "", cur); text[cur] = $0; next }
        cur != "" && NF { text[cur] = text[cur] " " $0 }
        END { for (i = 1; i <= 7; i++) printf "%d\t%s\n", i, text[i] }
    ' "$learnings")
}

# Les commandes que le manifeste exige avant de déclarer une étape finie.
check_commands() {
    local dir="$1" label log
    log=$(mktemp)
    run() {
        label="$1"
        shift
        if (cd "$dir" && "$@") > "$log" 2>&1; then
            ok "$label"
        else
            err "$label"
            grep -E '(^|: )(error|warning)(\[|:)|^Diff in|^test .* FAILED|^thread .* panicked' "$log" | grep -v 'could not compile' | head -8 | sed 's/^/      /'
        fi
    }
    run "cargo fmt --check" cargo fmt --check
    run "cargo clippy --all-targets" cargo clippy --all-targets --message-format=short -- -D warnings
    run "cargo clippy --target wasm32-unknown-unknown" cargo clippy --target wasm32-unknown-unknown --message-format=short -- -D warnings
    run "cargo test" cargo test
    rm -f "$log"
}

# CERTIFICATION.md : un jeu décrit au harnais comment le lancer et le rejoindre.
check_cert() {
    local dir="$1" problem
    case "$dir" in "$ROOT"/games/*) ;; *) return ;; esac
    if [ ! -f "$dir/cert.json" ]; then
        warn "cert.json absent : le harnais de CERTIFICATION.md ne sait pas lancer ce jeu"
        return
    fi
    problem=$(node -e '
        const c = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
        const missing = ["launch", "ready", "tv"].filter((k) => !c[k]);
        if (!("phone" in c)) missing.push("phone (null sans téléphones)");
        if (c.phone && !c.joined) missing.push("joined");
        if (c.phone && !Array.isArray(c.join)) missing.push("join");
        if (missing.length) console.log("il manque " + missing.join(", "));
    ' "$dir/cert.json" 2>&1)
    if [ -n "$problem" ]; then
        err "cert.json : ${problem%%$'\n'*}"
    else
        ok "cert.json lisible par le harnais (node tools/cert/cert.mjs ${dir#"$ROOT"/})"
    fi
}

check_project() {
    local dir="$1"
    printf '\n== %s\n' "${dir#"$ROOT"/}"
    if [ ! -f "$dir/Cargo.toml" ]; then
        err "pas de Cargo.toml"
        return
    fi
    check_toolchain "$dir"
    check_cargo "$dir"
    check_clippy_toml "$dir"
    check_headless_core "$dir"
    check_javascript "$dir"
    check_doc_paths "$dir"
    check_gaps "$dir"
    check_cert "$dir"
    [ "$FAST" = 1 ] || check_commands "$dir"
}

projects=()
for arg in "$@"; do
    case "$arg" in
        --fast) FAST=1 ;;
        -h | --help)
            sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) projects+=("$(cd "$arg" && pwd)") ;;
    esac
done
if [ "${#projects[@]}" = 0 ]; then
    while IFS= read -r toolchain; do
        projects+=("$(dirname "$toolchain")")
    done < <(find "$ROOT" -maxdepth 3 -name rust-toolchain.toml -not -path '*/target/*' | sort)
fi

for project in "${projects[@]}"; do
    check_project "$project"
done

printf '\n%d erreur(s), %d avertissement(s)\n' "$ERRORS" "$WARNINGS"
[ "$ERRORS" = 0 ]
