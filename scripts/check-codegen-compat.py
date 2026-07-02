#!/usr/bin/env python3
# Copyright (C) Parity Technologies (UK) Ltd.
# SPDX-License-Identifier: GPL-3.0-only
"""Guard that the committed zombienet-sdk test codegens stay SCALE-compatible with the relay-clients
codegens for the bridge types the relayer exchanges with the nodes.

Why: `substrate-relay` is built from the `relay-clients/client-*/src/codegen_runtime.rs` modules
(generated from the *live production* chains). The zombienet-sdk tests spawn nodes at the polkadot-sdk
revision pinned in `Cargo.lock` and drive that same relayer against them, decoding/encoding via
`testing/zombienet-sdk-tests/tests/codegen/*.rs` (generated `--full` from the pinned runtimes). If a
bridge wire type drifts between the two, the relayer silently mis-encodes against the test nodes and
the zombienet suite breaks in a way that's hard to diagnose. This static check catches that up front.

What "compatible" means (SCALE semantics, so path/attribute spelling is irrelevant):
  - structs/tuples: identical field order, names and (path-stripped) leaf types;
  - enums: every codec index present in BOTH sides must have the same variant name+fields. Extra
    indices on either side are additive and allowed (e.g. a new pallet Call variant on master) --
    reported as info, not a failure.
Types present on only one side (subxt full-vs-types-only output, or genuine version skew) are info.

Usage: scripts/check-codegen-compat.py            # check all mapped chains, exit 1 on incompatibility
       scripts/check-codegen-compat.py --verbose   # also print the per-chain info notes
"""
import os, re, sys

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ZN_DIR = os.path.join(REPO_ROOT, "testing/zombienet-sdk-tests/tests/codegen")

# The bridge modules whose types make up the relayer <-> node wire format.
BRIDGE_MODS = [
    "bp_header_chain", "bp_messages", "bp_parachains", "bp_relayers", "bp_runtime",
    "bp_xcm_bridge_hub", "pallet_bridge_grandpa", "pallet_bridge_messages",
    "pallet_bridge_parachains", "pallet_bridge_relayers", "pallet_xcm_bridge_hub",
]


def read(path):
    with open(path) as f:
        return f.read()


def basename_types(s):
    """Collapse any `a::b::C` path to its last segment and drop whitespace, so that substitution /
    path spelling differences (`::sp_weights::Weight` vs `runtime_types::sp_weights::weight_v2::Weight`)
    reduce to the same SCALE-relevant skeleton."""
    s = re.sub(r"\s+", "", s)
    s = re.sub(r"(?:[A-Za-z_][A-Za-z0-9_]*::)+([A-Za-z_][A-Za-z0-9_]*)", r"\1", s)
    return s.replace("::", "")


def strip_attrs_keep_codec(src):
    """Drop every `#[..]` attribute (balanced, possibly multi-line) except `#[codec(..)]`, which
    carries enum variant indices."""
    out, i, n = [], 0, len(src)
    while i < n:
        if src.startswith("#[", i):
            j, depth = i + 2, 1
            while j < n and depth:
                if src[j] == "[":
                    depth += 1
                elif src[j] == "]":
                    depth -= 1
                j += 1
            if src[i:j].startswith("#[codec"):
                out.append(src[i:j])
            i = j
        else:
            out.append(src[i])
            i += 1
    return "".join(out)


def _match_balanced(src, start, open_ch, close_ch):
    """Given src[start] == open_ch, return index just past the matching close_ch."""
    depth, i, n = 1, start + 1, len(src)
    while i < n and depth:
        if src[i] == open_ch:
            depth += 1
        elif src[i] == close_ch:
            depth -= 1
        i += 1
    return i


def parse_enum_variants(body):
    """Return {index: 'VariantName<fields>'} for a `#[codec(index=..)]`-annotated enum body."""
    variants, i, n = {}, 0, len(body)
    for m in re.finditer(r"#\[codec\(index\s*=\s*(\d+)\)\]\s*([A-Za-z_]\w*)", body):
        idx, name = int(m.group(1)), m.group(2)
        k = m.end()
        payload = ""
        if k < n and body[k] == "(":
            end = _match_balanced(body, k, "(", ")")
            payload = body[k:end]
        elif k < n and body[k] == "{":
            end = _match_balanced(body, k, "{", "}")
            payload = body[k:end]
        variants[idx] = name + basename_types(payload)
    return variants


def collect_types(body, prefix, out):
    """Recursively collect struct/enum signatures under a module body into `out[qualified_name]`."""
    i, n = 0, len(body)
    while i < n:
        tail = body[i:]
        mmod = re.match(r"\s*pub mod ([A-Za-z_]\w*)\s*\{", tail)
        menum = re.match(r"\s*pub enum ([A-Za-z_]\w*)\s*\{", tail)
        mstruct = re.match(r"\s*pub struct ([A-Za-z_]\w*)", tail)
        if mmod:
            s = _match_balanced(body, i + mmod.end() - 1, "{", "}")
            collect_types(body[i + mmod.end():s - 1], prefix + mmod.group(1) + "::", out)
            i = s
        elif menum:
            s = _match_balanced(body, i + menum.end() - 1, "{", "}")
            out[prefix + menum.group(1)] = ("enum", parse_enum_variants(body[i + menum.end():s - 1]))
            i = s
        elif mstruct:
            s = i + mstruct.end()
            rest = body[s:]
            if re.match(r"\s*\{", rest):
                op = s + rest.index("{")
                e = _match_balanced(body, op, "{", "}")
                out[prefix + mstruct.group(1)] = ("struct", basename_types(body[op + 1:e - 1]))
                i = e
            elif re.match(r"\s*\(", rest):
                op = s + rest.index("(")
                e = _match_balanced(body, op, "(", ")")
                out[prefix + mstruct.group(1)] = ("tuple", basename_types(body[op + 1:e - 1]))
                i = e
            else:
                out[prefix + mstruct.group(1)] = ("unit", "")
                i = s + 1
        else:
            i += 1
    return out


def signatures(path):
    src = strip_attrs_keep_codec(read(path))
    out = {}
    for mod in BRIDGE_MODS:
        m = re.search(r"\bpub mod %s\s*\{" % re.escape(mod), src)
        if not m:
            continue
        end = _match_balanced(src, m.end() - 1, "{", "}")
        collect_types(src[m.end():end - 1], mod + "::", out)
    return out


def compare(name, a, b, incompat, info):
    (ka, va), (kb, vb) = a, b
    if ka != kb:
        incompat.append(f"{name}: kind changed ({ka} -> {kb})")
        return
    if ka == "enum":
        shared = set(va) & set(vb)
        bad = [i for i in sorted(shared) if va[i] != vb[i]]
        for i in bad:
            incompat.append(f"{name}: variant #{i} differs (relay-clients={va[i]!r} zombienet={vb[i]!r})")
        extra = (set(va) - set(vb)) | (set(vb) - set(va))
        if extra:
            info.append(f"{name}: additive variant index(es) {sorted(extra)} (compatible)")
    elif va != vb:
        incompat.append(f"{name}: struct/tuple shape differs (relay-clients={va!r} zombienet={vb!r})")


def check_pair(chain, rc_path, zn_path, verbose):
    a, b = signatures(rc_path), signatures(zn_path)
    shared = sorted(set(a) & set(b))
    incompat, info = [], []
    for t in shared:
        compare(t, a[t], b[t], incompat, info)
    only_a, only_b = sorted(set(a) - set(b)), sorted(set(b) - set(a))
    status = "INCOMPATIBLE" if incompat else "ok"
    print(f"[{status}] {chain}: {len(shared)} shared bridge types checked")
    for line in incompat:
        print(f"    ✗ {line}")
    if verbose:
        for line in info:
            print(f"    · {line}")
        if only_a:
            print(f"    · only in relay-clients: {', '.join(only_a)}")
        if only_b:
            print(f"    · only in zombienet:     {', '.join(only_b)}")
    return not incompat


def main():
    verbose = "--verbose" in sys.argv[1:]
    files = sorted(f for f in os.listdir(ZN_DIR) if f.endswith(".rs"))
    all_ok, checked = True, 0
    for f in files:
        chain = f[:-3]                                   # e.g. bridge_hub_rococo
        rc = os.path.join(REPO_ROOT, "relay-clients",
                          "client-" + chain.replace("_", "-"), "src", "codegen_runtime.rs")
        zn = os.path.join(ZN_DIR, f)
        if not os.path.exists(rc):
            print(f"[skip] {chain}: no relay-clients counterpart ({os.path.relpath(rc, REPO_ROOT)})")
            continue
        checked += 1
        all_ok &= check_pair(chain, rc, zn, verbose)
    if checked == 0:
        print("ERROR: no chain pairs were checked", file=sys.stderr)
        return 2
    print("\nRESULT:", "COMPATIBLE" if all_ok else "INCOMPATIBLE — regenerate the stale codegen")
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())
