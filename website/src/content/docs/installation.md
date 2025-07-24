---
title: Installation
description: How to install cuenv on your system
---

## Installation Methods

### Using Nix Flake (Recommended)

The recommended way to install cuenv is through the Nix flake:

```bash
# Run directly without installing
nix run github:rawkode/cuenv

# Install into your profile
nix profile install github:rawkode/cuenv

# Or add to your flake.nix
```

```nix title="flake.nix"
{
  inputs.cuenv.url = "github:rawkode/cuenv";
  # ... use cuenv.packages.${system}.default
}
```

### Using Cargo

Install from crates.io using Cargo:

```bash
cargo install cuenv
```

### From GitHub Releases

Download the pre-built binary from the [releases page](https://github.com/rawkode/cuenv/releases):

```bash
# Example for Linux x86_64
curl -L https://github.com/rawkode/cuenv/releases/latest/download/cuenv-linux-x86_64 -o cuenv
chmod +x cuenv
sudo mv cuenv /usr/local/bin/
```

### Building from Source

If you need to build from source, use Nix for a reproducible build:

```bash
# Clone the repository
git clone https://github.com/rawkode/cuenv.git
cd cuenv

# Build with Nix
nix build

# The binary will be in ./result/bin/cuenv
./result/bin/cuenv --version
```

## Shell Setup

After installation, you need to configure your shell to use cuenv. Add the appropriate initialization to your shell configuration file:

### Bash

Add to `~/.bashrc`:

```bash title="~/.bashrc"
eval "$(cuenv init bash)"
```

### Zsh

Add to `~/.zshrc`:

```zsh title="~/.zshrc"
eval "$(cuenv init zsh)"
```

### Fish

Add to `~/.config/fish/config.fish`:

```fish title="~/.config/fish/config.fish"
cuenv init fish | source
```

## Verify Installation

After restarting your shell or sourcing your configuration file, verify the installation:

```bash
# Check if cuenv is installed
cuenv --version

# Create a test env.cue file
echo 'package env
TEST_VAR: "Hello from cuenv!"' > env.cue

# The environment should load automatically
echo $TEST_VAR
# Output: Hello from cuenv!

# Clean up
rm env.cue
```

## Secret Manager Setup (Optional)

### 1Password

Install the [1Password CLI](https://developer.1password.com/docs/cli/) and authenticate:

```bash
# Install 1Password CLI
# See https://developer.1password.com/docs/cli/get-started/

# Sign in
op signin
```

### Google Cloud Platform

Install the [gcloud CLI](https://cloud.google.com/sdk/docs/install) and authenticate:

```bash
# Install gcloud CLI
# See https://cloud.google.com/sdk/docs/install

# Authenticate
gcloud auth login
```

## Next Steps

- Follow the [Quick Start](/quickstart/) guide
- Learn about [CUE file format](/guides/cue-format/)
- Explore [secret management](/guides/secrets/)