# AWS Secrets Manager Integration

This example demonstrates how to integrate cuenv with AWS Secrets Manager for secure secret management.

## Prerequisites

1. Install [AWS CLI](https://aws.amazon.com/cli/)
2. Configure AWS credentials:
   ```bash
   aws configure
   # or use IAM roles, instance profiles, or environment variables
   ```

## Configuration

The `env.cue` file demonstrates:

- **Simple secrets**: Using `aws secretsmanager get-secret-value`
- **Regional secrets**: Specifying different AWS regions
- **JSON secret extraction**: Using `jq` to extract fields from JSON secrets
- **Cross-account access**: Using full ARNs for secrets in other accounts
- **Environment-specific secrets**: Different secret paths for each environment

## Usage

```bash
# Ensure AWS credentials are configured
aws sts get-caller-identity

# Run application with resolved secrets
cuenv run -- my-application

# Use specific environment
cuenv run -e production -- my-application

# Use specific AWS profile
AWS_PROFILE=myprofile cuenv run -- my-application
```

## AWS Secrets Manager Setup

```bash
# Create a simple secret
aws secretsmanager create-secret \
    --name "myapp/database/password" \
    --description "Database password for MyApp" \
    --secret-string "my-secure-password"

# Create a JSON secret
aws secretsmanager create-secret \
    --name "myapp/oauth" \
    --description "OAuth credentials" \
    --secret-string '{"client_id":"abc123","client_secret":"xyz789"}'

# Create environment-specific secrets
aws secretsmanager create-secret \
    --name "dev/myapp/database" \
    --secret-string "dev-password"

aws secretsmanager create-secret \
    --name "prod/myapp/database" \
    --secret-string "super-secure-prod-password"
```

## IAM Permissions

Your AWS credentials need the following permissions:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Action": [
                "secretsmanager:GetSecretValue"
            ],
            "Resource": [
                "arn:aws:secretsmanager:*:*:secret:myapp/*",
                "arn:aws:secretsmanager:*:*:secret:dev/*",
                "arn:aws:secretsmanager:*:*:secret:prod/*"
            ]
        }
    ]
}
```