# Validate a committed development candidate

A candidate is an immutable feature-branch commit. Preparation, local Windows
checks, remote host checks and independent pilot observations are separate facts.
Candidate evidence never replaces the stable release collector's six successful
`main` push runs or published upgrade/rollback checks.

Commit the reviewed source and retain `git rev-parse HEAD`. Push the ordinary
`codex/` branch and open a draft pull request. The six existing gates and
toolchain distribution run on pull requests. Their checkouts explicitly select
the pull request's head commit and assert it in `Verify candidate checkout`;
they do not combine results from a moving merge revision. Packaging receives
that same revision. Changes after the candidate commit require a new candidate.

The workflows also support manual dispatch once their dispatch declarations are
available on the repository's default branch. Dispatch each existing workflow
with `gh workflow run <workflow.yml> --ref <candidate-branch>`. GitHub may reject
dispatch before the default-branch workflow supports it; the draft pull request
is the available trigger for this implementation. Creating a draft does not
publish or merge the candidate.

After the checks finish, capture actual GitHub API results:

```powershell
$candidate = git rev-parse HEAD
python scripts/candidate_evidence.py collect `
  --repo Hyper66666/Sengoo --ref codex/daily-development-python-ecosystem `
  --sha $candidate --output target/candidate-evidence.json
python scripts/candidate_evidence.py validate target/candidate-evidence.json
```

Use `--gh <absolute-path>` if GitHub CLI is outside PATH. Collection retains
pending and failed results and exits nonzero while qualification is incomplete.
It chooses the newest run and current attempt for each workflow; an older green
run cannot hide a newer failure. Artifacts from an earlier attempt are retained
separately and cannot satisfy a rerun's requirements. API failures and missing workflows remain
explicit. Authentication is through the user's GitHub CLI session; tokens do
not belong in evidence files or command arguments.

Use **Re-run all jobs** when qualifying a new attempt. A partial retry that
inherits older successful hosts or their artifacts does not satisfy the
current-attempt completeness rule.

The candidate validator requires each of the four native targets and each of
the CPython 3.11–3.14 consumers. It checks exact source/repository/branch,
successful checkout identity, mandatory test steps, jobs, artifact identity, nonempty content and
expiry. A skipped required host fails. Only the distribution workflow's tag-only
publication and published-transition jobs may remain skipped; they are recorded
as deferred release work. The report is not a release signature or independent
verification of GitHub itself.

Generated ownership failures retain seed, source, IR and command transcripts.
Incremental index reports include zero-write auditing and transaction recovery.
Python wheel reports include real batch results, native resource counters and
observed conversion/call costs. Keep all reports with their matching artifacts;
the candidate collector also retains GitHub run and artifact URLs. Download
needed artifacts before their expiry, preserving downloaded archive checksums.

Independent pilot status is **deferred by the owner, not started**. The
[pilot kit](daily-development-pilot.md) remains ready for a later explicit
invitation. Internal checks do not count as participant observations.
