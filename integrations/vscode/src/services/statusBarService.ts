import * as vscode from 'vscode';
import { EnvironmentManager } from './environmentManager';
import { EnvironmentState } from '../types/environment';
import { Logger } from './logger';

export class StatusBarService {
    private statusBarItem: vscode.StatusBarItem;
    private environmentManagers = new Map<string, EnvironmentManager>();
    private disposables: vscode.Disposable[] = [];

    constructor(private logger: Logger) {
        this.statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            100
        );
        this.statusBarItem.command = 'cuenv.showQuickPick';
        
        // Listen to active editor changes to update context
        this.disposables.push(
            vscode.window.onDidChangeActiveTextEditor(() => this.updateStatusBar())
        );

        this.updateStatusBar();
    }

    addEnvironmentManager(workspaceFolder: vscode.WorkspaceFolder, manager: EnvironmentManager): void {
        const key = workspaceFolder.uri.fsPath;
        this.environmentManagers.set(key, manager);
        
        // Listen to state changes
        this.disposables.push(
            manager.onDidChangeEnvironment(() => this.updateStatusBar())
        );
        
        this.updateStatusBar();
    }

    private getCurrentWorkspaceFolder(): vscode.WorkspaceFolder | undefined {
        const activeEditor = vscode.window.activeTextEditor;
        if (activeEditor) {
            return vscode.workspace.getWorkspaceFolder(activeEditor.document.uri);
        }
        
        // Fall back to first workspace folder if no active editor
        return vscode.workspace.workspaceFolders?.[0];
    }

    private updateStatusBar(): void {
        const currentFolder = this.getCurrentWorkspaceFolder();
        if (!currentFolder) {
            this.statusBarItem.hide();
            return;
        }

        const manager = this.environmentManagers.get(currentFolder.uri.fsPath);
        if (!manager) {
            this.statusBarItem.hide();
            return;
        }

        const state = manager.state;
        const { icon, text, tooltip } = this.getStatusBarInfo(state, currentFolder.name);
        
        this.statusBarItem.text = `${icon} ${text}`;
        this.statusBarItem.tooltip = tooltip;
        this.statusBarItem.show();

        // Set context for when clause in package.json
        vscode.commands.executeCommand('setContext', 'cuenv.hasEnvFile', 
            state !== EnvironmentState.NotFound);
    }

    private getStatusBarInfo(state: EnvironmentState, folderName: string): {
        icon: string;
        text: string;
        tooltip: string;
    } {
        switch (state) {
            case EnvironmentState.Loaded:
                return {
                    icon: '$(check)',
                    text: 'cuenv',
                    tooltip: `cuenv: Environment loaded for ${folderName}`
                };
            case EnvironmentState.PendingReload:
                return {
                    icon: '$(sync)',
                    text: 'cuenv',
                    tooltip: `cuenv: Environment needs reload for ${folderName}`
                };
            case EnvironmentState.Error:
                return {
                    icon: '$(warning)',
                    text: 'cuenv',
                    tooltip: `cuenv: Error loading environment for ${folderName}`
                };
            case EnvironmentState.BinaryNotFound:
                return {
                    icon: '$(error)',
                    text: 'cuenv',
                    tooltip: 'cuenv: Binary not found'
                };
            case EnvironmentState.Disabled:
                return {
                    icon: '$(circle-slash)',
                    text: 'cuenv',
                    tooltip: 'cuenv: Auto-load disabled'
                };
            case EnvironmentState.NotFound:
            default:
                return {
                    icon: '$(circle-outline)',
                    text: 'cuenv',
                    tooltip: 'cuenv: No env.cue file found'
                };
        }
    }

    async showQuickPick(): Promise<void> {
        const currentFolder = this.getCurrentWorkspaceFolder();
        if (!currentFolder) {
            return;
        }

        const manager = this.environmentManagers.get(currentFolder.uri.fsPath);
        if (!manager) {
            return;
        }

        const items: vscode.QuickPickItem[] = [
            {
                label: '$(refresh) Reload Environment',
                description: 'Reload the cuenv environment',
                detail: `Reload environment for ${currentFolder.name}`
            },
            {
                label: '$(output) Open cuenv Output',
                description: 'Show the cuenv output channel',
                detail: 'View logs and debug information'
            },
            {
                label: '$(toggle) Toggle Auto Load',
                description: 'Toggle automatic environment loading',
                detail: 'Enable or disable auto-loading on startup'
            }
        ];

        // Add reveal env.cue option if file exists
        if (manager.state !== EnvironmentState.NotFound) {
            items.push({
                label: '$(file-code) Reveal env.cue',
                description: 'Open the env.cue configuration file',
                detail: 'Edit environment configuration'
            });
        }

        const selected = await vscode.window.showQuickPick(items, {
            placeHolder: 'Choose an action'
        });

        if (selected) {
            await this.handleQuickPickSelection(selected, manager, currentFolder);
        }
    }

    private async handleQuickPickSelection(
        item: vscode.QuickPickItem,
        manager: EnvironmentManager,
        folder: vscode.WorkspaceFolder
    ): Promise<void> {
        if (item.label.includes('Reload Environment')) {
            await manager.reload();
        } else if (item.label.includes('Open cuenv Output')) {
            this.logger.show();
        } else if (item.label.includes('Toggle Auto Load')) {
            await vscode.commands.executeCommand('cuenv.toggleAutoLoad');
        } else if (item.label.includes('Reveal env.cue')) {
            const envCueUri = vscode.Uri.joinPath(folder.uri, 'env.cue');
            try {
                const document = await vscode.workspace.openTextDocument(envCueUri);
                await vscode.window.showTextDocument(document);
            } catch (error) {
                this.logger.error('Failed to open env.cue', error);
            }
        }
    }

    dispose(): void {
        this.statusBarItem.dispose();
        this.disposables.forEach(d => d.dispose());
    }
}