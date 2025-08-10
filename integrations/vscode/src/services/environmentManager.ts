import * as vscode from 'vscode';
import * as path from 'path';
import { EventEmitter, Event } from 'vscode';
import { CLIAdapter } from './cliAdapter';
import { Logger } from './logger';
import { EnvironmentState, EnvironmentJson } from '../types/environment';
import { hashFile } from '../util/hashing';

export class EnvironmentManager {
    private _onDidChangeEnvironment = new EventEmitter<EnvironmentState>();
    private _state: EnvironmentState = EnvironmentState.NotFound;
    private _environment: EnvironmentJson | null = null;
    private _lastFileHash: string | null = null;
    private _fileWatcher: vscode.FileSystemWatcher | null = null;
    private _debounceTimer: NodeJS.Timer | null = null;
    private _disposables: vscode.Disposable[] = [];

    constructor(
        private workspaceFolder: vscode.WorkspaceFolder,
        private cliAdapter: CLIAdapter,
        private logger: Logger,
        private debounceMs: number = 300
    ) {
        this.setupFileWatcher();
    }

    get onDidChangeEnvironment(): Event<EnvironmentState> {
        return this._onDidChangeEnvironment.event;
    }

    get state(): EnvironmentState {
        return this._state;
    }

    get environment(): EnvironmentJson | null {
        return this._environment;
    }

    private get envCuePath(): string {
        return path.join(this.workspaceFolder.uri.fsPath, 'env.cue');
    }

    private setState(newState: EnvironmentState): void {
        if (this._state !== newState) {
            this._state = newState;
            this._onDidChangeEnvironment.fire(newState);
            this.logger.debug(`Environment state changed to: ${newState} for ${this.workspaceFolder.name}`);
        }
    }

    private setupFileWatcher(): void {
        const pattern = new vscode.RelativePattern(this.workspaceFolder, 'env.cue');
        this._fileWatcher = vscode.workspace.createFileSystemWatcher(pattern);
        
        this._disposables.push(
            this._fileWatcher,
            this._fileWatcher.onDidChange(() => this.onFileChanged()),
            this._fileWatcher.onDidCreate(() => this.onFileChanged()),
            this._fileWatcher.onDidDelete(() => this.onFileDeleted())
        );
    }

    private onFileChanged(): void {
        // Debounce file changes to avoid excessive reloads
        if (this._debounceTimer) {
            clearTimeout(this._debounceTimer);
        }

        this._debounceTimer = setTimeout(async () => {
            this._debounceTimer = null;
            await this.detectChanges();
        }, this.debounceMs);
    }

    private onFileDeleted(): void {
        this.setState(EnvironmentState.NotFound);
        this._environment = null;
        this._lastFileHash = null;
    }

    async load(): Promise<void> {
        try {
            // Check if binary exists
            const binaryExists = await this.cliAdapter.checkBinaryExists();
            if (!binaryExists) {
                this.setState(EnvironmentState.BinaryNotFound);
                return;
            }

            // Check if env.cue exists
            try {
                await vscode.workspace.fs.stat(vscode.Uri.file(this.envCuePath));
            } catch {
                this.setState(EnvironmentState.NotFound);
                return;
            }

            // Load environment
            const environment = await this.cliAdapter.exportEnv(this.workspaceFolder.uri.fsPath);
            this._environment = environment;
            
            // Update file hash for change detection
            this._lastFileHash = await hashFile(this.envCuePath);
            
            this.setState(EnvironmentState.Loaded);
            this.logger.info(`Environment loaded for ${this.workspaceFolder.name}`);
            
        } catch (error) {
            this.setState(EnvironmentState.Error);
            this.logger.error(`Failed to load environment for ${this.workspaceFolder.name}`, error);
        }
    }

    async reload(): Promise<void> {
        this.logger.info(`Reloading environment for ${this.workspaceFolder.name}`);
        await this.load();
    }

    async detectChanges(): Promise<void> {
        if (this._state === EnvironmentState.NotFound || this._state === EnvironmentState.BinaryNotFound) {
            // Try loading if we were previously in an error state
            await this.load();
            return;
        }

        const currentHash = await hashFile(this.envCuePath);
        if (currentHash && currentHash !== this._lastFileHash) {
            this.setState(EnvironmentState.PendingReload);
            this.showReloadPrompt();
        }
    }

    private showReloadPrompt(): void {
        const action = 'Reload Now';
        vscode.window.showInformationMessage(
            `env.cue has changed in ${this.workspaceFolder.name}. Would you like to reload the environment?`,
            action,
            'Dismiss'
        ).then(selection => {
            if (selection === action) {
                this.reload();
            }
        });
    }

    updateDebounceMs(newDebounceMs: number): void {
        this.debounceMs = newDebounceMs;
    }

    updateExecutablePath(newPath: string): void {
        this.cliAdapter.updateExecutablePath(newPath);
    }

    dispose(): void {
        if (this._debounceTimer) {
            clearTimeout(this._debounceTimer);
        }
        this._onDidChangeEnvironment.dispose();
        this._disposables.forEach(d => d.dispose());
    }
}