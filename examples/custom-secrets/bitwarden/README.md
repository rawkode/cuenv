# Bitwarden CLI Integration

This example demonstrates how to integrate cuenv with the Bitwarden CLI for secret management.

## Prerequisites

1. Install [Bitwarden CLI](https://bitwarden.com/help/cli/)
2. Authenticate with Bitwarden:
   ```bash
   bw login your-email@example.com
   bw unlock  # Enter your master password
   export BW_SESSION="your-session-key"
   ```

## Configuration

The `env.cue` file demonstrates:

- **Simple passwords**: Using `bw get password`
- **Custom fields**: Extracting specific fields from items
- **JSON processing**: Using `jq` to extract complex field values
- **TOTP codes**: Getting time-based one-time passwords
- **Organization vaults**: Different vaults for different environments

## Usage

```bash
# Verify Bitwarden authentication
bw status

# If locked, unlock and export session
bw unlock
export BW_SESSION="your-session-key"

# Run application with resolved secrets
cuenv run -- my-application

# Use specific environment
cuenv run -e production -- my-application
```

## Bitwarden Setup

### Create Items in Bitwarden

1. **Database Item**:
   - Name: "MyApp Database"
   - Username: "db_user"
   - Password: "secure_password"

2. **API Key Item**:
   - Name: "Stripe API"
   - Add custom field: "api_key" = "sk_live_..."

3. **OAuth Client**:
   - Name: "OAuth Client"
   - Add custom fields:
     - "client_id" = "your_client_id"
     - "client_secret" = "your_client_secret"

4. **Email SMTP**:
   - Name: "Email SMTP"
   - Username: "smtp_username"
   - Password: "smtp_password"

### Organization Setup

For production environments, use Bitwarden Organizations:

```bash
# List organizations
bw list organizations

# Get items from specific organization
bw get item "Production Database" --organizationid "org-id"
```

## Security Notes

- **Session management**: Sessions expire and need to be refreshed
- **Master password**: Keep your master password secure
- **Two-factor authentication**: Enable 2FA on your Bitwarden account
- **Organization policies**: Use organization policies for team access control
- **Vault isolation**: Use separate vaults/organizations for different environments

## Troubleshooting

**Session expired:**
```bash
bw unlock
export BW_SESSION="new-session-key"
```

**Item not found:**
```bash
# Search for items
bw list items --search "database"

# Check exact item name
bw get item "exact-item-name"
```

**Organization access:**
```bash
# List your organizations
bw list organizations

# Verify organization access
bw list items --organizationid "org-id"
```