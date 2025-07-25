# Unix Pass Integration

This example shows how to use cuenv with the Unix password manager [pass](https://www.passwordstore.org/).

## Prerequisites

1. Install [pass](https://www.passwordstore.org/)
2. Initialize password store:
   ```bash
   gpg --generate-key  # if you don't have a GPG key
   pass init your-gpg-id@example.com
   ```

## Configuration

The `env.cue` file demonstrates:

- **Simple passwords**: Using `pass` to retrieve passwords
- **Multi-line secrets**: Storing SSH keys and certificates
- **Structured secrets**: Extracting specific lines from pass entries
- **Environment-specific paths**: Using pass directory structure for environments

## Usage

```bash
# Verify pass is working
pass ls

# Run application with resolved secrets
cuenv run -- my-application

# Use specific environment
cuenv run -e production -- my-application
```

## Pass Setup Example

```bash
# Create directory structure
pass insert myapp/dev/database
pass insert myapp/staging/database
pass insert myapp/prod/database

# Add API keys
pass insert external/stripe/live-api-key
pass insert external/stripe/test-api-key

# Add multi-line secrets (SSH keys, etc.)
pass insert -m infrastructure/ssh/deploy-key

# Add structured email credentials
pass insert email/smtp
# Then edit to have username on first line, password on second:
# user@example.com
# smtp-password-123

# List all passwords
pass ls
```

## Directory Structure

A typical pass store structure for this example:

```
~/.password-store/
├── myapp/
│   ├── dev/
│   │   └── database.gpg
│   ├── staging/
│   │   └── database.gpg
│   └── prod/
│       └── database.gpg
├── external/
│   └── stripe/
│       ├── live-api-key.gpg
│       └── test-api-key.gpg
├── infrastructure/
│   └── ssh/
│       └── deploy-key.gpg
└── email/
    └── smtp.gpg
```

## Security Benefits

- **GPG encryption**: All passwords encrypted with your GPG key
- **Git versioning**: Optional git integration for password history
- **Simple CLI**: Familiar Unix tool philosophy
- **No network dependencies**: Fully offline password management
- **Team sharing**: Can be shared via git repositories with team GPG keys