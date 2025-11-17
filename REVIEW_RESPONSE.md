# Review Feedback Response

## ✅ Current Implementation Status

### Timer-Based Reconciliation ✅ IMPLEMENTED
- **Status**: `next_reconcile_time` is set in status (lines 2583-2593, 2886-2896 in reconciler.rs)
- **Mechanism**: `Action::requeue(duration)` triggers reconciliation after reconcile_interval
- **Detection**: Main watch loop checks if `next_reconcile_time` has passed (lines 910-943 in main.rs)
- **Drift Detection**: Periodic reconciliations run even when generation hasn't changed (line 951-952)
- **Verification Needed**: Ensure timer-based resync works when nothing changes (drift detection)

### Error Policy Separation ⚠️ NEEDS WORK
- **Current**: Backoff logic in reconciler (lines 675-705)
- **TODO**: Move to error_policy() layer (line 709-710)
- **Risk**: Could block watch/timer paths if many resources fail

### Artifact Edge-Cases ⚠️ NEEDS WORK
- **Current**: Basic download/extract implemented
- **Missing**: Corrupt file handling, partial download recovery, invalid checksum handling
- **Missing**: Temp dir cleanup, path-traversal security

### Status Fields ✅ IMPLEMENTED
- **Current**: `next_reconcile_time` exists in status
- **Enhancement**: Could add `NextScheduledReconcileAt` for better visibility

### Observability ⚠️ PARTIAL
- **Current**: Duration metrics, reconciliation counters
- **Missing**: Secrets published counter, skipped secrets counter, requeue count
- **Missing**: OTEL spans around fetch/extract/publish paths

## 🔧 Priority Action Items

1. **Verify timer-based resync works for drift detection** (HIGH)
2. **Move backoff logic to error_policy()** (HIGH - addresses deadlock risk)
3. **Add artifact edge-case handling** (MEDIUM)
4. **Add observability metrics** (MEDIUM)
5. **RBAC security review** (MEDIUM)
6. **Performance/scalability** (LOW)
7. **E2E tests** (LOW)
8. **Documentation** (LOW)
