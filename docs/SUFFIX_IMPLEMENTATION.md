# Secret Suffix and Character Sanitization Implementation

## Summary

This document describes the implementation of secret suffix support and character sanitization to match the `kustomize-google-secret-manager` implementation, enabling our controller to be a drop-in replacement.

## Changes Made

### 1. Added `secretSuffix` Field

**CRD Specification (`src/main.rs`):**
- Added `secret_suffix: Option<String>` field to `SecretManagerConfigSpec`
- Matches the prior art's suffix behavior

**CRD YAML (`config/crd/secretmanagerconfig.yaml`):**
- Added `secretSuffix` field to the OpenAPI schema
- Field is nullable/optional

**CRD Generator (`src/crdgen.rs`):**
- Added `secret_suffix` field to match main.rs structure

### 2. Secret Name Construction

**New Helper Function (`src/reconciler.rs`):**
```rust
fn construct_secret_name(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String
```

**Naming Convention:**
- `{prefix}-{key}-{suffix}` if both prefix and suffix exist
- `{prefix}-{key}` if only prefix exists
- `{key}-{suffix}` if only suffix exists
- `{key}` if neither exists

**Character Sanitization:**
```rust
fn sanitize_secret_name(name: &str) -> String
```

Replaces invalid characters:
- `.` → `_`
- `/` → `_`
- Spaces → `_`
- Any other invalid character → `_`

Keeps valid characters: `[a-zA-Z0-9_-]+`

### 3. Updated Secret Name Construction Points

All secret name construction now uses the new helper function:

1. **`process_application_files`** - Raw file mode secrets
2. **`process_kustomize_secrets`** - Kustomize build mode secrets
3. **Properties secret** - JSON-encoded properties

### 4. Documentation Updates

- **README.md**: Updated secret naming section with suffix examples
- **examples/README.md**: Added suffix documentation
- **examples/idam-dev-secret-manager-config.yaml**: Added suffix example (commented)
- **docs/GAP_ANALYSIS.md**: Marked suffix and sanitization as complete

## Usage Examples

### With Prefix Only (Existing Behavior)
```yaml
spec:
  secretPrefix: my-service
```

Results in: `my-service-database-url`, `my-service-api-key`

### With Prefix and Suffix (New)
```yaml
spec:
  secretPrefix: my-service
  secretSuffix: -prod
```

Results in: `my-service-database-url-prod`, `my-service-api-key-prod`

### With Suffix Only
```yaml
spec:
  secretSuffix: -prod
```

Results in: `database-url-prod`, `api-key-prod`

### Character Sanitization Examples

| Input | Output | Reason |
|-------|--------|--------|
| `my.service/db-url` | `my_service_db-url` | `.` and `/` replaced |
| `my service key` | `my_service_key` | Spaces replaced |
| `my-service-key` | `my-service-key` | Valid, unchanged |

## Compatibility

### Drop-in Replacement

Our controller now matches the `kustomize-google-secret-manager` naming conventions:

✅ **Prefix Support** - Matches prior art  
✅ **Suffix Support** - Matches prior art  
✅ **Character Sanitization** - Matches prior art  
✅ **Naming Pattern** - Matches prior art format

### Migration Path

Users migrating from `kustomize-google-secret-manager` can:

1. **Keep existing prefix/suffix configuration** - No changes needed
2. **Use same secret names** - Secrets will be named identically
3. **No application changes** - Applications consuming secrets don't need updates

### Example Migration

**Before (kustomize-google-secret-manager):**
```yaml
# Kustomize plugin config
prefix: my-service
suffix: -prod
```

**After (Our Controller):**
```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
spec:
  secretPrefix: my-service
  secretSuffix: -prod
```

**Result:** Identical secret names in GCP Secret Manager

## Testing

### Unit Tests Needed

1. **`construct_secret_name` function:**
   - Test with prefix only
   - Test with suffix only
   - Test with both prefix and suffix
   - Test with neither (should use key as-is)

2. **`sanitize_secret_name` function:**
   - Test `.` replacement
   - Test `/` replacement
   - Test space replacement
   - Test other invalid characters
   - Test valid characters remain unchanged

3. **Integration Tests:**
   - Test secret creation with prefix/suffix
   - Test secret name matches expected format
   - Test character sanitization in real GCP Secret Manager

## Verification

To verify the implementation:

1. **Check CRD:**
   ```bash
   kubectl get crd secretmanagerconfigs.secret-management.microscaler.io -o yaml | grep -A 5 secretSuffix
   ```

2. **Test Secret Name Construction:**
   ```rust
   // In tests
   assert_eq!(
       construct_secret_name(Some("my-service"), "db-url", Some("-prod")),
       "my-service-db-url-prod"
   );
   ```

3. **Test Character Sanitization:**
   ```rust
   assert_eq!(
       sanitize_secret_name("my.service/db-url"),
       "my_service_db-url"
   );
   ```

## Next Steps

1. ✅ Add `secretSuffix` field - **Complete**
2. ✅ Implement character sanitization - **Complete**
3. ✅ Update all secret name construction - **Complete**
4. ✅ Update documentation - **Complete**
5. ⏳ Add unit tests - **Pending**
6. ⏳ Add integration tests - **Pending**
7. ⏳ Complete GCP Secret Manager client implementation - **Pending**

## Related Files

- `src/main.rs` - CRD definition with `secret_suffix` field
- `src/reconciler.rs` - Secret name construction logic
- `src/crdgen.rs` - CRD generator with suffix field
- `config/crd/secretmanagerconfig.yaml` - CRD YAML schema
- `README.md` - Updated documentation
- `examples/README.md` - Updated examples documentation
- `docs/GAP_ANALYSIS.md` - Gap analysis with implementation status

