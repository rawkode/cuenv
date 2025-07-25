# SOPS (Secrets OPerationS) Integration

This example shows how to use cuenv with Mozilla SOPS for file-based secret management with encryption.

## Prerequisites

1. Install [SOPS](https://github.com/mozilla/sops)
2. Set up encryption keys (choose one):
   - **age**: `age-keygen -o ~/.config/sops/age/keys.txt`
   - **GPG**: `gpg --generate-key`
   - **Cloud KMS**: Configure AWS KMS, GCP KMS, or Azure Key Vault

## Configuration

Create a `.sops.yaml` file in your project:

```yaml
creation_rules:
  - path_regex: secrets/dev\.yaml$
    age: age1abcdef...
  - path_regex: secrets/prod\.yaml$
    age: age1xyz789...
  - path_regex: secrets\.yaml$
    age: age1default...
```

## Sample Encrypted Files

Create and encrypt secret files:

```bash
# Create development secrets
cat > secrets/dev.yaml << EOF
database:
  password: "dev-password"
  host: "dev-db.example.com"
api_key: "dev-api-key"
jwt_secret: "dev-jwt-secret"
EOF

# Encrypt the file
sops --encrypt --in-place secrets/dev.yaml

# Create production secrets
cat > secrets/prod.yaml << EOF
database:
  password: "super-secure-prod-password"
  host: "prod-db.example.com"
api_key: "live-api-key"
jwt_secret: "prod-jwt-secret"
EOF

sops --encrypt --in-place secrets/prod.yaml
```

## Usage

```bash
# Verify SOPS can decrypt
sops --decrypt secrets/dev.yaml

# Run application with cuenv
cuenv run -e development -- my-application

# Production environment
cuenv run -e production -- my-application
```

## Security Benefits

- **Encryption at rest**: Secret files are encrypted in git repositories
- **Key rotation**: Easy to rotate encryption keys
- **Audit trail**: Git history shows who changed what
- **Multi-backend**: Supports age, GPG, AWS KMS, GCP KMS, Azure Key Vault
- **Selective encryption**: Only encrypt specific fields in YAML/JSON files

## File Formats

SOPS supports multiple formats:

```yaml
# YAML format (secrets.yaml)
database:
  password: "secret"
api_key: "key"
```

```json
// JSON format (secrets.json)
{
  "database": {
    "password": "secret"
  },
  "api_key": "key"
}
```