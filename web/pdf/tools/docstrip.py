#!/usr/bin/env python3
"""Minimal deterministic docstrip emulator (extract-only, no TeX required).

Package the songbook's TeX inputs from their upstream docstrip sources:

  songs.dtx + songs.ins  ->  songs.sty     (module `package`)
  classes.dtx + classes.ins -> size12.clo  (module `12pt`)

The emulator reproduces the validated spike reference outputs byte-for-byte
(see /tmp/opencode/ref/ and /tmp/opencode/pdf-spike-validation.md).  Known,
deliberate deviation from real docstrip: the .ins preamble block is NOT
prepended to generated files (the validated references omit it; the preamble
text is recorded in the build manifest instead).  Generation is verified
against pinned SHA-256 hashes of the reference outputs and fails hard on
mismatch so a silent source drift can never ship.

Usage: docstrip.py <job.json>
  job.json = {
    "sources": {"songs.dtx": {"path": ..., "sha256": ...}, ...},
    "jobs": [{"output": "songs.sty", "dtx": "songs.dtx", "ins": "songs.ins",
              "module": "package", "expect_sha256": "..."}]
  }
"""
import hashlib
import json
import re
import sys

FILE_RE = re.compile(r"\\file\{([^{}]+)\}\{\\from\{([^{}]+)\}\{([^{}]+)\}\}")
OPEN_RE = re.compile(rb"^%<\*([a-zA-Z0-9_+,\-|&!]+)>[ \t]*$")
CLOSE_RE = re.compile(rb"^%</([a-zA-Z0-9_+,\-|&!]+)>[ \t]*$")
INLINE_RE = re.compile(rb"^%<([a-zA-Z0-9_+,\-|&!]+)>(.*)$")


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def eval_guard(expr, target):
    """Evaluate a docstrip guard expression against a target module set.

    Grammar: alternatives separated by `|` (OR of AND-terms); terms separated
    by `&`; each atom may be negated with a leading `!`.  `*` matches any
    target.  Returns True iff the expression is true for `target`.
    """
    for alt in expr.split("|"):
        ok = True
        for atom in alt.split("&"):
            atom = atom.strip()
            if not atom:
                continue
            neg = atom.startswith("!")
            if neg:
                atom = atom[1:]
            if atom == "*":
                hit = True
            else:
                hit = atom in target
            if neg:
                hit = not hit
            if not hit:
                ok = False
                break
        if ok:
            return True
    return False


def docstrip_extract(dtx_bytes, module):
    """Extract the module's code lines from docstrip .dtx bytes."""
    targets = {m.strip() for m in module.split(",") if m.strip()}
    out = []
    stack = []  # stack of guard expressions currently open
    for line in dtx_bytes.split(b"\n"):
        if line.startswith(b"%"):
            m = OPEN_RE.match(line)
            if m:
                stack.append(m.group(1))
                continue
            m = CLOSE_RE.match(line)
            if m:
                # docstrip semantics: a close guard ends the innermost open
                # guard block with the *same* expression (tag-aware, not a
                # plain LIFO pop — classes.dtx relies on this).
                tag = m.group(1)
                for k in range(len(stack) - 1, -1, -1):
                    if stack[k] == tag:
                        del stack[k]
                        break
                else:
                    print(
                        "docstrip: warning: unmatched close guard %r"
                        % line.decode("latin-1"),
                        file=sys.stderr,
                    )
                continue
            m = INLINE_RE.match(line)
            if m and eval_guard(m.group(1).decode("latin-1"), targets) and all(
                eval_guard(g.decode("latin-1"), targets) for g in stack
            ):
                out.append(m.group(2))
                continue
            # plain meta-comment: drop
            continue
        # code line: emitted iff target is in every open guard expression
        if all(eval_guard(g.decode("latin-1"), targets) for g in stack):
            out.append(line)
    return out


def parse_ins(ins_bytes):
    """Extract (preamble lines, {output: (dtx, module)}) from an .ins file."""
    preamble = []
    state = "normal"
    files = {}
    for line in ins_bytes.decode("latin-1").splitlines():
        if state == "normal":
            if line.strip() == "\\preamble":
                state = "preamble"
                continue
            for m in FILE_RE.finditer(line):
                files[m.group(1)] = (m.group(2), m.group(3))
        elif state == "preamble":
            if line.strip() == "\\endpreamble":
                state = "normal"
                continue
            preamble.append(line)
    return preamble, files


def run_job(job, sources):
    dtx_name = job["dtx"]
    ins_name = job.get("ins")
    dtx_path = sources[dtx_name]["path"]
    dtx_bytes = open(dtx_path, "rb").read()
    want = sources[dtx_name].get("sha256")
    got = hashlib.sha256(dtx_bytes).hexdigest()
    if want and want != got:
        raise SystemExit(
            "docstrip: source %s hash mismatch (expected %s, got %s)"
            % (dtx_name, want, got)
        )
    ins_info = None
    if ins_name:
        ins_bytes = open(sources[ins_name]["path"], "rb").read()
        ins_info = parse_ins(ins_bytes)
        # the ins must actually request this output from this dtx/module
        match = ins_info[1].get(job["output"])
        if not match:
            raise SystemExit(
                "docstrip: %s does not declare output %s" % (ins_name, job["output"])
            )
        if match[0] != dtx_name or match[1] != job["module"]:
            raise SystemExit(
                "docstrip: %s declares %s from %s{%s}; job says %s{%s}"
                % (ins_name, job["output"], match[0], match[1], dtx_name, job["module"])
            )
    lines = docstrip_extract(dtx_bytes, job["module"])
    data = b"\n".join(lines)
    if not data.endswith(b"\n"):
        data += b"\n"
    out_path = job["output"]
    with open(out_path, "wb") as f:
        f.write(data)
    digest = hashlib.sha256(data).hexdigest()
    expect = job.get("expect_sha256")
    if expect and digest != expect:
        raise SystemExit(
            "docstrip: output %s sha256 mismatch\n  expected %s\n  generated %s"
            % (out_path, expect, digest)
        )
    print(
        "docstrip: generated %s (%d bytes) from %s module %s%s"
        % (
            out_path,
            len(data),
            dtx_name,
            job["module"],
            " [preamble recorded, not applied]" if ins_info else "",
        )
    )
    return {
        "output": out_path,
        "dtx": dtx_name,
        "ins": ins_name,
        "module": job["module"],
        "sha256": digest,
        "bytes": len(data),
        "preamble_lines": len(ins_info[0]) if ins_info else 0,
    }


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    with open(sys.argv[1]) as f:
        job = json.load(f)
    results = [run_job(j, job["sources"]) for j in job["jobs"]]
    with open(job.get("report", "docstrip-report.json"), "w") as f:
        json.dump(results, f, indent=2)
    return 0


if __name__ == "__main__":
    sys.exit(main())
