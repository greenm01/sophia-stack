#!/bin/sh
# Refuse a reader that can no longer match the record its emitter writes today.
#
# Evidence records are emitted by production code and read by verifiers,
# reporters, and gates. When an emitter's schema is bumped, a reader whose
# acceptance pattern names only older schemas stops matching. Nothing fails:
# the reader finds no line, the block it guards is skipped, and the rule it
# owned silently stops running. That has now happened three times to one
# record. The one-KMS-submission bound and the renderer-misroute check were
# both guarded by `== 9`/`== 10` equality and so went unasserted for every
# schema-11 and schema-12 session; the QEMU shared-worker expectation was
# pinned to `schema=10` exactly and could not run at all.
#
# A guarded pattern may accept older schemas as well -- archives stay
# independently verifiable, which is why the alternations are long -- but it
# may not accept *only* older ones.
#
# Guarded records are named explicitly below rather than discovered. A record
# name alone does not identify a message: `sophia_live_wm` writes schema 4 for
# `status=ready` and schema 1 for `status=session_action_committed`, and
# `sophia_session_app` writes schema 1 and schema 2 for the same status under
# different sources. Guarding a record therefore requires knowing that its
# emitters agree, which holds for the records listed here. Add a record when
# that has been checked, not before.
#
# Fixture builders under tools/check_*.sh are excluded: they write synthetic
# old-schema evidence on purpose, to prove a verifier still accepts an archive.
set -eu

repo_root="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

python3 - <<'PY'
import os
import re
import sys

# Records whose emitters have been checked to write one schema per message.
GUARDED = (
    'sophia_live_native_resources',
    'sophia_live_rendering_efficiency',
)

emitted = {}
for root, dirs, files in os.walk('crates'):
    dirs[:] = [d for d in dirs if d != 'target']
    if os.sep + 'src' not in root + os.sep:
        continue
    for name in files:
        if not name.endswith('.rs'):
            continue
        with open(os.path.join(root, name), encoding='utf-8', errors='replace') as handle:
            for line in handle:
                for record in GUARDED:
                    for match in re.finditer(re.escape(record) + r' schema=([0-9]+)', line):
                        schema = int(match.group(1))
                        if schema > emitted.get(record, 0):
                            emitted[record] = schema

missing = [record for record in GUARDED if record not in emitted]
if missing:
    print('no emitter writes %s; the guard names a record that no longer exists'
          % ', '.join(missing), file=sys.stderr)
    sys.exit(1)

reader_re = re.compile(
    r'(%s) schema=([^ \\/\'"]+)' % '|'.join(re.escape(r) for r in GUARDED))

failures = []
checked = 0
for root, dirs, files in os.walk('tools'):
    dirs[:] = [d for d in dirs if d != 'fixtures']
    for name in sorted(files):
        if not (name.endswith('.sh') or name.endswith('.py')):
            continue
        if name.startswith('check_'):
            continue
        path = os.path.join(root, name)
        with open(path, encoding='utf-8', errors='replace') as handle:
            for number, line in enumerate(handle, start=1):
                for match in reader_re.finditer(line):
                    record, token = match.group(1), match.group(2)
                    current = str(emitted[record])
                    try:
                        pattern = re.compile(token + r'\Z')
                    except re.error:
                        failures.append('%s:%d: %s reader pattern %s is not a regex'
                                        % (path, number, record, token))
                        continue
                    checked += 1
                    if not pattern.match(current):
                        failures.append(
                            '%s:%d: %s reader accepts schema=%s but the emitter '
                            'writes schema=%s'
                            % (path, number, record, token, current))

for failure in failures:
    print(failure, file=sys.stderr)

print('checked %d reader(s) of %d guarded record(s)' % (checked, len(GUARDED)))
if failures:
    print('%d reader(s) cannot match the record their emitter writes today'
          % len(failures), file=sys.stderr)
    sys.exit(1)
PY
