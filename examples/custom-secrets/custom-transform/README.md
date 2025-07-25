# Custom Transformation Examples

This example demonstrates advanced patterns for custom secret processing, including transformations, validations, and complex integrations.

## Use Cases

This example covers:

- **Base64 encoding/decoding**: Transform secret formats
- **Secret validation**: Ensure secrets meet requirements
- **Composite secrets**: Combine multiple sources into one value
- **HTTP API integration**: Fetch secrets from web APIs
- **Time-based rotation**: Check and warn about old secrets
- **Environment-specific logic**: Different behavior per environment

## Examples Explained

### Base64 Decoding

```cue
DECODED_SECRET: {
    resolver: {
        command: "sh"
        args: [
            "-c",
            "echo 'aGVsbG8gd29ybGQ=' | base64 -d"
        ]
    }
}
```

Useful for secrets stored in base64 format.

### Secret Validation

```cue
VALIDATED_KEY: {
    resolver: {
        command: "sh"
        args: [
            "-c",
            '''
            key=$(vault kv get -field=api_key secret/myapp)
            if [[ ${#key} -lt 32 ]]; then
                echo "Error: API key too short" >&2
                exit 1
            fi
            echo "$key"
            '''
        ]
    }
}
```

Validates that API keys meet length requirements before using them.

### Composite Secrets

```cue
DATABASE_URL: {
    resolver: {
        command: "sh"
        args: [
            "-c",
            '''
            user=$(vault kv get -field=username secret/db)
            pass=$(vault kv get -field=password secret/db)
            host=$(consul kv get database/host)
            echo "postgres://$user:$pass@$host:5432/myapp"
            '''
        ]
    }
}
```

Combines username, password, and host from different sources into a connection string.

### HTTP API Integration

```cue
API_SECRET: {
    resolver: {
        command: "curl"
        args: [
            "-s",
            "-H", "Authorization: Bearer $VAULT_TOKEN",
            "https://vault.company.com/v1/secret/data/myapp",
            "|", "jq", "-r", ".data.data.secret"
        ]
    }
}
```

Fetches secrets from REST APIs with authentication.

### Time-based Rotation Check

```cue
ROTATED_SECRET: {
    resolver: {
        command: "sh"
        args: [
            "-c",
            '''
            # Check if secret needs rotation (older than 30 days)
            last_updated=$(vault kv metadata get -field=created_time secret/myapp/key)
            current=$(date +%s)
            updated=$(date -d "$last_updated" +%s)
            age_days=$(( (current - updated) / 86400 ))
            
            if [[ $age_days -gt 30 ]]; then
                echo "Warning: Secret is $age_days days old" >&2
            fi
            
            vault kv get -field=value secret/myapp/key
            '''
        ]
    }
}
```

Checks secret age and warns when rotation is needed.

## Best Practices

### Error Handling

- Always exit with non-zero status on errors
- Write error messages to stderr
- Validate inputs before processing

### Shell Safety

```bash
# Use quotes to prevent word splitting
echo "$var"

# Use arrays for complex commands
args=("--flag" "$value")
command "${args[@]}"

# Check command existence
if ! command -v jq >/dev/null; then
    echo "Error: jq not found" >&2
    exit 1
fi
```

### Security Considerations

- Avoid echoing secrets to stdout/stderr during processing
- Use temporary files carefully (prefer pipes)
- Validate all external inputs
- Use minimal required permissions

## Usage

```bash
# Ensure all required tools are available
which vault curl jq openssl

# Run with transformations
cuenv run -- my-application

# Test individual resolvers
vault kv get -field=api_key secret/myapp
curl -s "https://api.example.com/secret" | jq -r .value
```

## Advanced Patterns

### Conditional Logic

```bash
# Different behavior based on environment
if [[ "$ENVIRONMENT" == "production" ]]; then
    vault kv get -field=live_key secret/payments
else
    echo "sk_test_mock_key"
fi
```

### Retry Logic

```bash
# Retry failed API calls
for i in {1..3}; do
    if result=$(curl -s "https://api.example.com/secret"); then
        echo "$result" | jq -r .value
        exit 0
    fi
    sleep $((i * 2))
done
echo "Failed to fetch secret after 3 attempts" >&2
exit 1
```

### Caching

```bash
# Cache expensive operations
CACHE_FILE="/tmp/secret_cache_$$"
if [[ -f "$CACHE_FILE" && $(($(date +%s) - $(stat -c %Y "$CACHE_FILE"))) -lt 300 ]]; then
    cat "$CACHE_FILE"
else
    expensive_secret_operation > "$CACHE_FILE"
    cat "$CACHE_FILE"
fi
```