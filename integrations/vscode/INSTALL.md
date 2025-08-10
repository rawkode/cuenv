# Installation Guide for cuenv VSCode Extension

## Prerequisites

1. **Install Bun**: This project uses Bun for package management and running scripts.

   ```bash
   # macOS/Linux
   curl -fsSL https://bun.sh/install | bash

   # Windows (via PowerShell)
   powershell -c "irm bun.sh/install.ps1 | iex"
   ```

2. **Install cuenv CLI**: Required for the extension to function.

   Visit the [cuenv releases page](https://github.com/rawkode/cuenv/releases) to download the latest binary for your platform.

## Development Setup

1. **Clone the repository**:

   ```bash
   git clone https://github.com/rawkode/cuenv.git
   cd cuenv/integrations/vscode
   ```

2. **Install dependencies**:

   ```bash
   bun install
   ```

3. **Build the extension**:

   ```bash
   bun run compile
   ```

4. **Run in development mode**:
   - Open the project in VSCode
   - Press `F5` to launch the Extension Development Host
   - The extension will be available in the new VSCode window

## Building for Production

1. **Compile TypeScript**:

   ```bash
   bun run vscode:prepublish
   ```

2. **Package the extension** (requires vsce):

   ```bash
   # Install vsce globally with Bun
   bun add -g @vscode/vsce

   # Package the extension
   vsce package
   ```

## Installing the Extension

### From VSIX file:

1. Build the extension as described above
2. In VSCode, open Command Palette (`Cmd+Shift+P` or `Ctrl+Shift+P`)
3. Run "Extensions: Install from VSIX..."
4. Select the generated `.vsix` file

### From Source (Development):

1. Follow the Development Setup steps above
2. The extension will be available in the Extension Development Host

## Troubleshooting

### Bun not found

- Ensure Bun is installed and added to your PATH
- Restart your terminal after installation
- Verify installation: `bun --version`

### TypeScript compilation errors

- Ensure you're using the correct TypeScript version: `bun run tsc --version`
- Clear the output directory: `rm -rf out/`
- Reinstall dependencies: `rm -rf node_modules bun.lockb && bun install`

### Extension not loading

- Check that cuenv CLI is installed and accessible: `cuenv --version`
- Verify the extension is enabled in VSCode
- Check the Output panel (View → Output → Select "cuenv") for error messages

## Scripts

- `bun run compile` - Compile TypeScript to JavaScript
- `bun run watch` - Watch for changes and recompile
- `bun run vscode:prepublish` - Prepare for publishing

## System Requirements

- **VSCode**: Version 1.74.0 or higher
- **Node.js**: Runtime provided by Bun
- **cuenv CLI**: Must be installed separately
- **Operating System**: Windows, macOS, or Linux
