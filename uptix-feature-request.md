# Feature Request: Enhance `friendly_version` field with human-readable information

## Problem Statement

The current `friendly_version` field in `uptix.lock` is not particularly helpful for understanding what changed between updates. When reviewing diffs of `uptix.lock`, it's difficult to tell:

1. **For Docker images with `:latest` or `:stable` tags**: Whether the update is significant or minor
2. **For GitHub branches**: What actually changed between commits
3. **For all dependency types**: How old the previous version was vs. the new version

This becomes especially problematic when using automated tools (like AI-powered changelog generators) to summarize nightly updates, as the diff provides no semantic context.

## Current Behavior

### Docker images (`:latest`, `:stable`, etc.)
```json
"friendly_version": "sha256:5ae78cf2e6d8",
```
**Problem**: Truncated image digests are meaningless to humans and provide no information about what changed.

**Diff view**:
```diff
-      "friendly_version": "sha256:5ae78cf2e6d8",
+      "friendly_version": "sha256:449140e073d8",
```
This tells us nothing useful.

### GitHub branches
```json
"friendly_version": "6647842",
```
**Problem**: Just a 7-character commit SHA with no context about when it was committed or what changed.

**Diff view**:
```diff
-      "friendly_version": "6647842",
+      "friendly_version": "a066890",
```
This gives no indication of the time span or nature of the changes.

### GitHub releases
```json
"friendly_version": "v2.0.1",
```
**Current state**: ✅ These are already good!

## Proposed Enhancement

Add a **`timestamp` field** and enhance the **`friendly_version`** field to make diffs human-readable.

### Docker images (`:latest`, `:stable`, etc.)

**Proposed behavior**:
```json
"friendly_version": "2024.11.3",
"timestamp": "2024-11-03T10:23:45Z"
```

**Implementation strategy**:
1. Parse image labels for semantic version information (e.g., `org.opencontainers.image.version`)
2. Use image creation/build date as fallback for `friendly_version` (formatted as `YYYY-MM-DD`)
3. Add `timestamp` field with ISO 8601 timestamp of the image build/version release

**Diff view**:
```diff
-      "friendly_version": "2024.10.5",
+      "friendly_version": "2024.11.3",
-      "timestamp": "2024-10-27T08:15:30Z",
+      "timestamp": "2024-11-03T10:23:45Z",
```
Now it's clear: we went from version 2024.10.5 to 2024.11.3, and we can see the exact dates.

### GitHub branches

**Proposed behavior**:
```json
"friendly_version": "a066890 (Add temperature calibration)",
"timestamp": "2024-11-01T14:32:10Z"
```

**Implementation strategy**:
1. Format `friendly_version` as: `SHORT_SHA (commit message first line, truncated to ~50 chars)`
2. Add `timestamp` field with ISO 8601 timestamp of the commit

**Diff view**:
```diff
-      "friendly_version": "6647842 (Fix sensor polling bug)",
+      "friendly_version": "a066890 (Add temperature calibration)",
-      "timestamp": "2024-10-27T12:45:22Z",
+      "timestamp": "2024-11-01T14:32:10Z",
```
Now we can see: 5 days passed, what changed, and the exact timestamps.

### GitHub releases

**Proposed behavior**:
```json
"friendly_version": "v2.0.1",
"timestamp": "2024-11-01T09:15:30Z"
```

**Implementation strategy**:
1. Keep `friendly_version` as the release tag (already good!)
2. Add `timestamp` field with the release publication date

**Current state**: `friendly_version` is already excellent! Just needs the `timestamp` field added for consistency.

## Use Case

We run automated nightly updates that:
1. Run `uptix update` to update all dependencies
2. Generate derivation diffs to see what changed in the Nix closures
3. Use Claude to analyze the diffs and generate a human-readable PR summary

Currently, the AI summary can only describe closure size changes (e.g., "+50MB for activepieces") but cannot explain:
- Which version of activepieces we upgraded to
- Whether we went from a 2-month-old version to yesterday's version
- What features/fixes were included in the update

With enhanced `friendly_version` and `timestamp` fields, the AI (and humans reviewing the PR) could immediately understand:
- "Home Assistant: 2024.10.5 → 2024.11.3 (updated Oct 27 → Nov 3)"
- "rtl_433: updated from Sept 15 to Oct 27 (6 weeks of fixes)"
- "activepieces: ancient version from August → current release"

## Benefits

1. **Human reviewers** can quickly assess update significance by looking at the git diff
2. **Automated tools** (CI summaries, AI changelog generators) can provide meaningful context
3. **Debugging**: Easier to correlate issues with specific versions when dates are visible
4. **Audit trail**: Clear visibility into how current or outdated dependencies are

## Backward Compatibility

- The `resolved_version` field would remain unchanged (still the authoritative source)
- The `friendly_version` field would be enhanced with more useful information
- A new `timestamp` field would be added
- Existing tooling that ignores these fields would be unaffected

## Implementation Notes

For **Docker images**, accessing semantic versions may require:
- Pulling image manifests to read labels
- Possible performance impact (can be mitigated with caching)
- Fallback to image creation date if labels unavailable

For **GitHub branches**, accessing commit metadata requires:
- GitHub API calls or local git operations
- May hit rate limits for large dependency lists (can batch requests)
- Commit message parsing (handle multi-line messages gracefully)

Would be happy to contribute or help test this feature!
