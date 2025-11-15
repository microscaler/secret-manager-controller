# Next Steps Implementation Plan

## Current State ✅

### Completed
1. ✅ **Pact Contract Testing** - Comprehensive API coverage (45 tests)
   - GCP Secret Manager: 12 tests
   - AWS Secrets Manager: 13 tests
   - AWS Parameter Store: 6 tests (NEW)
   - Azure Key Vault: 14 tests
   - All tests passing and publishing to Pact broker

2. ✅ **Controller Infrastructure**
   - Controller deploying and running in Kubernetes
   - SOPS GPG key reading from Kubernetes secrets
   - FluxCD GitRepository integration working
   - ArgoCD Application integration working
   - Kustomize build mode working
   - Raw file mode working

3. ✅ **Parser Implementation**
   - Parses `application.secrets.env` → secrets
   - Parses `application.secrets.yaml` → secrets
   - Parses `application.properties` → properties
   - SOPS decryption working

4. ✅ **GCP Secret Manager Provider**
   - Full CRUD operations implemented
   - Version management
   - Secret lifecycle management

### Current Gap ✅ RESOLVED

**Properties routing to config stores** (IMPLEMENTED):
- ✅ AWS: Properties route to Parameter Store as individual parameters when `configs.enabled = true`
- ✅ GCP: Properties route to Secret Manager as individual secrets when `configs.enabled = true` (interim solution)
- ✅ Backward compatibility: Properties stored as JSON blob when `configs.enabled = false` (default)

## Next Implementation Phase: Config Store Routing

### Goal
Route `application.properties` to cloud config stores instead of secret stores, storing individual properties as separate entries.

### Implementation Priority

#### Phase 1: AWS Parameter Store (High Priority) ✅ COMPLETE
**Status**: ✅ Implemented  
**Effort**: Completed  
**Why First**: 
- AWS SDK already available (`aws-sdk-ssm`)
- Best EKS integration (ASCP mounts as files)
- Lower cost than Secrets Manager
- ESO already supports Parameter Store

**Tasks**:
1. ✅ Enable AWS provider (currently disabled)
2. ✅ Add `aws-sdk-ssm` dependency
3. ✅ Create `ConfigStoreProvider` trait
4. ✅ Implement `AwsParameterStore` provider
5. ✅ Update CRD schema with `configs` field
6. ✅ Update reconciler to route properties → Parameter Store
7. ✅ Store individual properties (not JSON blob)
8. ✅ Add Pact tests for Parameter Store API (6 tests)

**Storage Format**:
```
/my-service/prod/database.host = db.example.com
/my-service/prod/database.port = 5432
/my-service/prod/api.timeout = 30s
```

#### Phase 2: GCP Secret Manager Config Routing ✅ COMPLETE
**Status**: ✅ Implemented  
**Effort**: Completed  
**Why Second**: 
- Uses existing Secret Manager provider
- Quick win (no new SDK needed)
- ESO already supports Secret Manager
- Interim solution until Parameter Manager contribution

**Tasks**:
1. ✅ Update reconciler to route properties → Secret Manager
2. ✅ Store individual properties as separate secrets
3. ✅ Update CRD schema with `configs.enabled` field
4. ✅ Implementation verified (uses existing Secret Manager provider and tests)

**Storage Format**:
```
my-service-database-host-prod = db.example.com
my-service-database-port-prod = 5432
my-service-api-timeout-prod = 30s
```

**Note**: This is an interim solution. Long-term goal is to contribute GCP Parameter Manager support to External Secrets Operator.

#### Phase 3: Azure App Configuration (Medium Priority)
**Status**: Research needed  
**Effort**: 2-3 days (after SDK research)  
**Why Third**: 
- Need to verify SDK availability
- Lower priority than AWS/GCP

**Tasks**:
1. Research `azure-app-configuration` Rust SDK
2. Enable Azure provider (currently disabled)
3. Create `AzureAppConfiguration` provider
4. Update reconciler
5. Add Pact tests

**Storage Format**:
```
my-service:prod:database.host = db.example.com
my-service:prod:database.port = 5432
my-service:prod:api.timeout = 30s
```

## CRD Design

```yaml
apiVersion: secret-manager.microscaler.io/v1alpha1
kind: SecretManagerConfig
metadata:
  name: my-service-config
spec:
  # Existing fields
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: flux-system
  provider:
    type: aws  # aws | gcp | azure
    aws:
      region: us-east-1
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/profiles/dev
    prefix: my-service
    suffix: -prod
  
  # NEW: Config store configuration
  configs:
    # Enable config store sync (default: false for backward compatibility)
    enabled: true
    
    # AWS-specific: Parameter path prefix
    # Only applies when provider.type == aws
    parameterPath: /my-service/dev  # Optional: defaults to /{prefix}/{environment}
    
    # GCP-specific: Store type (default: SecretManager)
    # Only applies when provider.type == gcp
    store: SecretManager  # SecretManager (default) | ParameterManager (future)
    
    # Azure-specific: App Configuration endpoint
    # Only applies when provider.type == azure
    appConfigEndpoint: https://my-app-config.azconfig.io  # Optional: auto-detect
```

## File Routing Logic

### Current Behavior ❌
```rust
// All files → Secret Store
application.secrets.env → Secret Store ✅
application.secrets.yaml → Secret Store ✅
application.properties → Secret Store (as JSON blob) ❌
```

### Target Behavior ✅
```rust
// File-based routing
if file_name.contains("secrets") {
    → Secret Store ✅
} else if file_name == "application.properties" || file_name.contains("config") {
    if configs.enabled {
        → Config Store (based on provider) ✅
    } else {
        → Secret Store (backward compatibility) ✅
    }
}
```

## Implementation Steps

### Step 1: Enable AWS Provider
- [ ] Uncomment AWS provider code
- [ ] Fix any rustls/crypto provider issues
- [ ] Verify AWS provider works with existing secret sync

### Step 2: Create ConfigStoreProvider Trait
- [ ] Define trait in `src/provider/mod.rs`
- [ ] Methods: `create_or_update_config`, `get_config_value`, `delete_config`
- [ ] Similar to `SecretManagerProvider` trait

### Step 3: Implement AWS Parameter Store Provider
- [ ] Create `src/provider/aws/parameter_store.rs`
- [ ] Implement `ConfigStoreProvider` trait
- [ ] Use `aws-sdk-ssm` for Parameter Store operations
- [ ] Handle parameter path construction
- [ ] Add error handling

### Step 4: Update CRD Schema
- [ ] Add `configs` field to `SecretManagerConfig` spec
- [ ] Add `ConfigsSpec` struct with fields:
  - `enabled: bool` (default: false)
  - `parameter_path: Option<String>` (AWS)
  - `store: Option<ConfigStoreType>` (GCP)
  - `app_config_endpoint: Option<String>` (Azure)
- [ ] Regenerate CRD using `crdgen`

### Step 5: Update Reconciler
- [ ] Add config store routing logic
- [ ] Route `application.properties` → config store (if enabled)
- [ ] Store individual properties (not JSON blob)
- [ ] Maintain backward compatibility (`configs.enabled: false`)

### Step 6: Add Tests
- [ ] Unit tests for Parameter Store provider
- [ ] Integration tests for config routing
- [ ] Pact tests for Parameter Store API
- [ ] Test backward compatibility

### Step 7: GCP Secret Manager Config Routing
- [ ] Update reconciler to route properties → Secret Manager
- [ ] Store individual properties as separate secrets
- [ ] Update CRD with `configs.store` field
- [ ] Add tests

### Step 8: Azure App Configuration (After Research)
- [ ] Research SDK availability
- [ ] Enable Azure provider
- [ ] Implement `AzureAppConfiguration` provider
- [ ] Update reconciler
- [ ] Add tests

## Code Structure

### New Files
```
src/provider/
  mod.rs                    # Export ConfigStoreProvider trait
  aws/
    mod.rs                  # Export ParameterStore
    parameter_store.rs      # AWS Parameter Store implementation
  gcp/
    mod.rs                  # (No changes - reuse SecretManager)
  azure/
    mod.rs                  # Export AppConfiguration
    app_configuration.rs    # Azure App Configuration implementation
```

### Updated Files
```
src/lib.rs                  # Add ConfigStoreProvider trait
src/controller/reconciler.rs # Add config routing logic
src/controller/parser.rs    # (No changes - already parses properties)
```

## Testing Strategy

### Unit Tests
- Config store provider implementations
- File routing logic
- Parameter/key name construction
- CRD validation

### Integration Tests
- AWS Parameter Store sync
- GCP Secret Manager config sync
- Azure App Configuration sync
- Backward compatibility

### Pact Tests
- Add Parameter Store API contracts
- Add App Configuration API contracts
- Verify config store operations

## Backward Compatibility

✅ **No breaking changes**:
- `configs.enabled: false` by default
- Existing CRDs continue to work
- Properties still go to secret stores unless explicitly enabled

## Success Criteria

1. ✅ `application.properties` routes to config stores (when enabled)
2. ✅ Individual properties stored as separate entries (not JSON blob)
3. ✅ Backward compatibility maintained (`configs.enabled: false` by default)
4. ✅ All three providers supported (AWS, GCP, Azure)
5. ✅ Clear CRD configuration for routing decisions
6. ✅ Tests passing (unit, integration, Pact)
7. ✅ Documentation updated

## Timeline Estimate

- **Phase 1 (AWS Parameter Store)**: 2-3 days
- **Phase 2 (GCP Secret Manager)**: 1-2 days
- **Phase 3 (Azure App Configuration)**: 2-3 days (after SDK research)

**Total**: ~1-2 weeks for all three phases

## Next Immediate Actions

1. **Enable AWS provider** - Uncomment code and fix any issues
2. **Create ConfigStoreProvider trait** - Define interface
3. **Implement AWS Parameter Store** - First provider implementation
4. **Update reconciler** - Add config routing logic
5. **Update CRD** - Add configs field
6. **Add tests** - Comprehensive test coverage

## Future Enhancements

1. **GCP Parameter Manager** - After ESO contribution
2. **Config validation** - Validate config values before storing
3. **Config versioning** - Track config changes over time
4. **Config rollback** - Ability to rollback config changes
5. **Multi-environment configs** - Better handling of environment-specific configs

